//! DICOMweb servers… (DESIGN.md §12.8), in the palette's File group: the servers the user
//! added, a study search on one, and opening a series into series mode while it streams.
//!
//! A modal sheet with three pages:
//!
//! - **Servers**: the list, with Add, Edit, Remove and Browse. Nothing is sent to a server
//!   until the user added it and asks to browse it.
//! - **Server**: name, service root URL (`https://…/dicom-web`), and the sign-in (none, user
//!   name and password, or an access token). The list is saved in the config folder without
//!   passwords or tokens, which are kept in memory until Specular closes (no keychain yet).
//! - **Search**: patient name and ID, a date range, modality and accession; studies come a
//!   page at a time (Load more), and opening one lists its series under it. Patient names,
//!   IDs and accession numbers in the results stay hidden until "Show patient data" is on;
//!   the switch is off again whenever the sheet opens.
//!
//! Opening a series closes the sheet and streams it ([`stream`]): the middle slice first,
//! then the rest nearest the slice on screen, with a toast that counts and can stop it.

pub(super) mod send;
#[cfg(test)]
mod shots;
mod stream;
pub(super) mod view;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::Receiver;
use specular_dicomweb::servers::{DEFAULT_OUR_AE, DEFAULT_PORT, Retrieve, UrlProblem, check_url};
use specular_dicomweb::{
    DicomWeb, Page as RowPage, SeriesRow, Server, ServerKind, ServerList, StudyQuery, StudyRow,
};
use specular_gfx::{DrawList, Rect, Text};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_remote::{
    Auth, AuthKind, CancelToken, Client, ClientConfig, CredentialStore, Secret, SessionCredentials,
};
use specular_theme::Palette;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard};

use self::view::{
    A11Y_BASE, Button, ButtonView, Control, DialogView, Drawn, FieldId, FormItem, Hit, ListEntry,
    Page, Tone,
};
use super::App;
use super::integration::PasteTo;

/// Studies or series asked for at a time.
const PAGE_SIZE: u32 = 50;
/// C-FIND: the server may take a while to match.
const FIND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// DICOM over TLS's registered port.
const DEFAULT_TLS_PORT: u16 = 2762;

/// A form cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Field(FieldId),
    Auth,
    ShowPatient,
    /// DICOMweb or DIMSE.
    Kind,
    /// Send page: what to send, and whether anonymised.
    Scope,
    Anonymise,
    /// DIMSE: DICOM over TLS.
    Tls,
    /// DIMSE: C-GET or C-MOVE.
    Retrieve,
}

/// What a list row stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Server(usize),
    Study(usize),
    Series(usize, usize),
    MoreStudies,
    MoreSeries(usize),
}

/// A search's answer.
enum Outcome {
    Studies {
        append: bool,
        query: StudyQuery,
        /// `Err(None)`: stopped.
        res: Result<RowPage<StudyRow>, Option<String>>,
    },
    Series {
        study: String,
        append: bool,
        res: Result<RowPage<SeriesRow>, Option<String>>,
    },
    /// Verify: the server's name, and whether it answered.
    Verified {
        server: String,
        res: Result<(), String>,
    },
}

/// Who answers a search: QIDO-RS, or C-FIND on a DIMSE server.
enum Finder {
    Web(DicomWeb),
    Dimse(Server),
}

impl Finder {
    fn dimse_text(e: &specular_dimse::Error, s: &Server) -> Option<String> {
        let host = s.host.trim().to_owned();
        match e {
            specular_dimse::Error::Cancelled => None,
            specular_dimse::Error::NothingAccepted | specular_dimse::Error::NoContext(_) => {
                Some(t!("pacs-find-not-supported", host = host))
            }
            specular_dimse::Error::Status(c) => Some(t!(
                "pacs-find-status",
                host = host,
                code = format!("{c:04X}")
            )),
            e => Some(send::dimse_fatal(e, s)),
        }
    }

    fn web_text(e: &specular_dicomweb::Error, label: &str) -> Option<String> {
        (!e.is_cancelled()).then(|| error_text(e, label))
    }

    fn studies(
        &self,
        q: &StudyQuery,
        label: &str,
        cancel: &CancelToken,
    ) -> Result<RowPage<StudyRow>, Option<String>> {
        match self {
            Finder::Web(w) => w
                .search_studies(q, cancel)
                .map_err(|e| Self::web_text(&e, label)),
            Finder::Dimse(s) => send::dimse_options(s, FIND_TIMEOUT)
                .and_then(|o| {
                    specular_dicomweb::find::search_studies(&send::peer_of(s), &o, q, cancel)
                })
                .map_err(|e| Self::dimse_text(&e, s)),
        }
    }

    fn series(
        &self,
        study: &str,
        offset: u32,
        label: &str,
        cancel: &CancelToken,
    ) -> Result<RowPage<SeriesRow>, Option<String>> {
        match self {
            Finder::Web(w) => w
                .search_series(study, PAGE_SIZE, offset, cancel)
                .map_err(|e| Self::web_text(&e, label)),
            Finder::Dimse(s) => send::dimse_options(s, FIND_TIMEOUT)
                .and_then(|o| {
                    specular_dicomweb::find::search_series(
                        &send::peer_of(s),
                        &o,
                        study,
                        PAGE_SIZE,
                        offset,
                        cancel,
                    )
                })
                .map_err(|e| Self::dimse_text(&e, s)),
        }
    }
}

struct Job {
    rx: Receiver<Outcome>,
    cancel: CancelToken,
}

/// The Server page's form.
struct Edit {
    /// The server edited (`None`: a new one).
    id: Option<String>,
    name: LineEdit,
    url: LineEdit,
    auth: AuthKind,
    user: LineEdit,
    secret: LineEdit,
    /// A password or token is kept for this server: an empty field keeps it.
    has_secret: bool,
    kind: ServerKind,
    host: LineEdit,
    port: LineEdit,
    ae: LineEdit,
    our_ae: LineEdit,
    /// DIMSE: DICOM over TLS.
    tls: bool,
    /// DIMSE: C-GET or C-MOVE, and where a move's instances arrive.
    retrieve: Retrieve,
    receive_address: LineEdit,
    receive_port: LineEdit,
    /// Save was pressed: problems with the URL are shown.
    tried: bool,
}

/// The Search page.
#[derive(Default)]
struct Search {
    patient_name: LineEdit,
    patient_id: LineEdit,
    date_from: LineEdit,
    date_to: LineEdit,
    modality: LineEdit,
    accession: LineEdit,
    /// The last query sent (for Load more).
    query: Option<StudyQuery>,
    studies: Vec<StudyRow>,
    more: bool,
    /// The study whose series are listed, with them.
    open: Option<usize>,
    series: Vec<SeriesRow>,
    series_more: bool,
}

/// The open sheet.
pub(super) struct Dialog {
    page: Page,
    /// The server browsed or selected (index into the list).
    server: Option<usize>,
    edit: Option<Edit>,
    search: Search,
    show_patient: bool,
    selected: Option<Row>,
    focus: Option<usize>,
    scroll: usize,
    message: Option<(String, Tone)>,
    job: Option<Job>,
    drawn: Drawn,
    /// What an input method is composing in the focused field (IME preedit).
    preedit: String,
    /// Send to PACS (the Send page).
    send: Option<send::Sending>,
    /// The server Remove asks about (its id): the Servers page shows the question.
    removing: Option<String>,
}

impl Dialog {
    fn new() -> Dialog {
        Dialog {
            page: Page::Servers,
            server: None,
            edit: None,
            search: Search::default(),
            show_patient: false,
            selected: None,
            focus: None,
            scroll: 0,
            message: None,
            job: None,
            drawn: Drawn::default(),
            preedit: String::new(),
            send: None,
            removing: None,
        }
    }
}

/// The app's DICOMweb state.
pub(super) struct DicomWebState {
    pub dialog: Option<Dialog>,
    servers: ServerList,
    loaded: bool,
    /// Where the list is saved (`None`: kept in memory, as in screenshots).
    servers_file: Option<PathBuf>,
    /// The disk cache's folder (`None`: the app's cache folder).
    cache_dir: Option<PathBuf>,
    creds: Arc<SessionCredentials>,
    client: Option<Client>,
    stream: Option<stream::Stream>,
    /// `--show dicomweb…`: the synthetic server the demo servers point at.
    demo: Option<specular_dicomweb::fake::FakeServer>,
}

impl Default for DicomWebState {
    fn default() -> Self {
        DicomWebState {
            dialog: None,
            servers: ServerList::default(),
            loaded: false,
            servers_file: None,
            cache_dir: None,
            creds: Arc::new(SessionCredentials::default()),
            client: None,
            stream: None,
            demo: None,
        }
    }
}

impl std::fmt::Debug for DicomWebState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DicomWebState")
            .field("open", &self.dialog.is_some())
            .field("servers", &self.servers.servers.len())
            .field("streaming", &self.stream.is_some())
            .finish_non_exhaustive()
    }
}

fn auth_text(kind: AuthKind) -> String {
    match kind {
        AuthKind::None => t!("dicomweb-auth-none"),
        AuthKind::Basic => t!("dicomweb-auth-basic"),
        AuthKind::Bearer => t!("dicomweb-auth-bearer"),
    }
}

fn url_problem_text(p: UrlProblem) -> String {
    match p {
        UrlProblem::Empty => t!("dicomweb-url-empty"),
        UrlProblem::Scheme => t!("dicomweb-url-scheme"),
        UrlProblem::NoHost => t!("dicomweb-url-host"),
        UrlProblem::UserInfo => t!("dicomweb-url-userinfo"),
    }
}

/// `20250214` as `2025-02-14`; anything else as it is.
fn date_text(d: &str) -> String {
    if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..])
    } else {
        d.to_string()
    }
}

/// A DICOM person name (`Family^Given^Middle`) for reading: "Family, Given Middle".
fn person_text(n: &str) -> String {
    let mut parts = n.split('=').next().unwrap_or("").split('^');
    let family = parts.next().unwrap_or("").trim();
    let rest: Vec<&str> = parts.map(str::trim).filter(|s| !s.is_empty()).collect();
    if rest.is_empty() {
        family.to_string()
    } else if family.is_empty() {
        rest.join(" ")
    } else {
        format!("{family}, {}", rest.join(" "))
    }
}

/// Why a request failed, for people.
fn error_text(e: &specular_dicomweb::Error, server: &str) -> String {
    if e.is_network_off() {
        t!("network-off")
    } else if e.is_offline() {
        t!("dicomweb-error-offline", server = server.to_string())
    } else if e.is_auth() {
        t!("dicomweb-error-auth")
    } else if e.status() == Some(404) {
        t!("dicomweb-error-not-found")
    } else {
        t!(
            "dicomweb-error-other",
            reason = crate::error_text::reason(e)
        )
    }
}

fn study_entry(s: &StudyRow, show_patient: bool, expanded: bool, selected: bool) -> ListEntry {
    let mut title = vec![date_text(&s.date)];
    if !s.modalities.is_empty() {
        title.push(s.modalities.join(", "));
    }
    title.push(if s.description.trim().is_empty() {
        t!("dicomweb-study-untitled")
    } else {
        s.description.trim().to_string()
    });
    let counts = t!(
        "dicomweb-study-counts",
        series = i64::from(s.series_count.unwrap_or(0)),
        images = i64::from(s.instance_count.unwrap_or(0))
    );
    let detail = if show_patient {
        let mut d = vec![person_text(&s.patient.name)];
        if !s.patient.id.is_empty() {
            d.push(t!("dicomweb-patient-id", id = s.patient.id.clone()));
        }
        if !s.accession.is_empty() {
            d.push(t!("dicomweb-accession", number = s.accession.clone()));
        }
        d.push(counts);
        specular_i18n::dot_join(&d)
    } else {
        specular_i18n::dot_join(&[t!("dicomweb-patient-hidden"), counts])
    };
    ListEntry {
        title: specular_i18n::dot_join(&title),
        detail,
        depth: 0,
        selected,
        expanded,
        action: false,
        thumb: view::Thumb::None,
    }
}

fn series_entry(s: &SeriesRow, selected: bool) -> ListEntry {
    let mut title = Vec::new();
    if let Some(n) = s.number {
        title.push(format!("#{n}"));
    }
    if !s.modality.is_empty() {
        title.push(s.modality.clone());
    }
    title.push(if s.description.trim().is_empty() {
        t!("dicomweb-series-untitled")
    } else {
        s.description.trim().to_string()
    });
    ListEntry {
        title: specular_i18n::dot_join(&title),
        detail: t!(
            "dicomweb-series-images",
            count = i64::from(s.instance_count.unwrap_or(0))
        ),
        depth: 1,
        selected,
        expanded: false,
        action: false,
        thumb: view::Thumb::None,
    }
}

fn more_entry(depth: u8) -> ListEntry {
    ListEntry {
        title: t!("dicomweb-load-more"),
        depth,
        action: true,
        ..ListEntry::default()
    }
}

impl Dialog {
    fn slots(&self) -> Vec<Slot> {
        match self.page {
            Page::Servers => Vec::new(),
            Page::Send => match &self.send {
                Some(s) if matches!(s.stage, send::Stage::Choose) => {
                    let mut v = Vec::new();
                    if s.req.choices.len() > 1 {
                        v.push(Slot::Scope);
                    }
                    v.push(Slot::Anonymise);
                    v
                }
                _ => Vec::new(),
            },
            Page::Edit
                if self
                    .edit
                    .as_ref()
                    .is_some_and(|e| e.kind == ServerKind::Dimse) =>
            {
                let mut v = vec![
                    Slot::Field(FieldId::Name),
                    Slot::Kind,
                    Slot::Field(FieldId::Host),
                    Slot::Field(FieldId::Port),
                    Slot::Field(FieldId::AeTitle),
                    Slot::Field(FieldId::OurAe),
                    Slot::Tls,
                    Slot::Retrieve,
                ];
                if self
                    .edit
                    .as_ref()
                    .is_some_and(|e| e.retrieve == Retrieve::Move)
                {
                    v.extend([
                        Slot::Field(FieldId::ReceiveAddress),
                        Slot::Field(FieldId::ReceivePort),
                    ]);
                }
                v
            }
            Page::Edit => {
                let mut v = vec![
                    Slot::Field(FieldId::Name),
                    Slot::Kind,
                    Slot::Field(FieldId::Url),
                    Slot::Auth,
                ];
                match self.edit.as_ref().map(|e| e.auth) {
                    Some(AuthKind::Basic) => {
                        v.extend([Slot::Field(FieldId::User), Slot::Field(FieldId::Secret)])
                    }
                    Some(AuthKind::Bearer) => v.push(Slot::Field(FieldId::Secret)),
                    _ => {}
                }
                v
            }
            Page::Search => vec![
                Slot::Field(FieldId::PatientName),
                Slot::Field(FieldId::PatientId),
                Slot::Field(FieldId::DateFrom),
                Slot::Field(FieldId::DateTo),
                Slot::Field(FieldId::Modality),
                Slot::Field(FieldId::Accession),
                Slot::ShowPatient,
            ],
        }
    }

    fn rows(&self, servers: &ServerList) -> Vec<Row> {
        match self.page {
            // The question about removing one stands alone.
            Page::Servers if self.removing.is_some() => Vec::new(),
            Page::Servers => (0..servers.servers.len()).map(Row::Server).collect(),
            Page::Send
                if self
                    .send
                    .as_ref()
                    .is_some_and(|s| matches!(s.stage, send::Stage::Choose)) =>
            {
                (0..servers.servers.len()).map(Row::Server).collect()
            }
            Page::Edit | Page::Send => Vec::new(),
            Page::Search => {
                let s = &self.search;
                let mut v = Vec::new();
                for i in 0..s.studies.len() {
                    v.push(Row::Study(i));
                    if s.open == Some(i) {
                        v.extend((0..s.series.len()).map(|j| Row::Series(i, j)));
                        if s.series_more {
                            v.push(Row::MoreSeries(i));
                        }
                    }
                }
                if s.more {
                    v.push(Row::MoreStudies);
                }
                v
            }
        }
    }

    fn field(&self, id: FieldId) -> Option<&LineEdit> {
        let s = &self.search;
        let e = self.edit.as_ref();
        Some(match id {
            FieldId::Name => &e?.name,
            FieldId::Url => &e?.url,
            FieldId::User => &e?.user,
            FieldId::Secret => &e?.secret,
            FieldId::PatientName => &s.patient_name,
            FieldId::PatientId => &s.patient_id,
            FieldId::DateFrom => &s.date_from,
            FieldId::DateTo => &s.date_to,
            FieldId::Modality => &s.modality,
            FieldId::Accession => &s.accession,
            FieldId::Host => &e?.host,
            FieldId::Port => &e?.port,
            FieldId::AeTitle => &e?.ae,
            FieldId::OurAe => &e?.our_ae,
            FieldId::ReceiveAddress => &e?.receive_address,
            FieldId::ReceivePort => &e?.receive_port,
        })
    }

    fn field_mut(&mut self, id: FieldId) -> Option<&mut LineEdit> {
        let s = &mut self.search;
        let e = self.edit.as_mut();
        Some(match id {
            FieldId::Name => &mut e?.name,
            FieldId::Url => &mut e?.url,
            FieldId::User => &mut e?.user,
            FieldId::Secret => &mut e?.secret,
            FieldId::PatientName => &mut s.patient_name,
            FieldId::PatientId => &mut s.patient_id,
            FieldId::DateFrom => &mut s.date_from,
            FieldId::DateTo => &mut s.date_to,
            FieldId::Modality => &mut s.modality,
            FieldId::Accession => &mut s.accession,
            FieldId::Host => &mut e?.host,
            FieldId::Port => &mut e?.port,
            FieldId::AeTitle => &mut e?.ae,
            FieldId::OurAe => &mut e?.our_ae,
            FieldId::ReceiveAddress => &mut e?.receive_address,
            FieldId::ReceivePort => &mut e?.receive_port,
        })
    }

    fn focused_field_mut(&mut self, v: &DialogView) -> Option<&mut LineEdit> {
        let id = v.focused_field()?;
        self.field_mut(id)
    }

    fn query(&self) -> StudyQuery {
        let s = &self.search;
        StudyQuery {
            patient_name: s.patient_name.text().trim().to_string(),
            patient_id: s.patient_id.text().trim().to_string(),
            date_from: s.date_from.text().trim().to_string(),
            date_to: s.date_to.text().trim().to_string(),
            modality: s.modality.text().trim().to_ascii_uppercase(),
            accession: s.accession.text().trim().to_string(),
            limit: PAGE_SIZE,
            offset: 0,
        }
    }

    /// The URL as typed, checked.
    fn url_check(&self) -> Option<Result<String, UrlProblem>> {
        self.edit.as_ref().map(|e| check_url(e.url.text()))
    }

    /// The server as the form stands (DICOMweb: its URL checked; DIMSE: every field).
    fn form_server(&self) -> Option<Result<Server, String>> {
        let e = self.edit.as_ref()?;
        if e.kind == ServerKind::Dimse {
            return self.dimse_check();
        }
        Some(match check_url(e.url.text()) {
            Ok(base_url) => Ok(Server {
                id: e.id.clone().unwrap_or_default(),
                name: e.name.text().trim().to_string(),
                base_url,
                auth: e.auth,
                user: e.user.text().trim().to_string(),
                ..Server::default()
            }),
            Err(p) => Err(url_problem_text(p)),
        })
    }

    /// A DIMSE server's fields, checked: the server, or what is wrong.
    fn dimse_check(&self) -> Option<Result<Server, String>> {
        let e = self.edit.as_ref().filter(|e| e.kind == ServerKind::Dimse)?;
        let host = e.host.text().trim();
        if host.is_empty() || host.contains(char::is_whitespace) {
            return Some(Err(t!("dicomweb-problem-host")));
        }
        let port = match e.port.text().trim().parse::<u16>() {
            Ok(p) if p > 0 => p,
            _ => return Some(Err(t!("dicomweb-problem-port"))),
        };
        let ae = e.ae.text().trim();
        let our = e.our_ae.text().trim();
        if !specular_dimse::pdu::valid_ae(ae)
            || (!our.is_empty() && !specular_dimse::pdu::valid_ae(our))
        {
            return Some(Err(t!("dicomweb-problem-ae")));
        }
        // C-MOVE: where the server sends to (an address of this computer and a port).
        let address = e.receive_address.text().trim();
        let receive_port = e.receive_port.text().trim();
        let receive_port = match receive_port.parse::<u16>() {
            Ok(p) if p > 0 => p,
            _ if receive_port.is_empty() => 0,
            _ => return Some(Err(t!("dicomweb-problem-port"))),
        };
        if e.retrieve == Retrieve::Move {
            if address.parse::<std::net::IpAddr>().is_err() {
                return Some(Err(t!("pacs-problem-receive-address")));
            }
            if receive_port == 0 {
                return Some(Err(t!("pacs-problem-receive-port")));
            }
        }
        Some(Ok(Server {
            id: e.id.clone().unwrap_or_default(),
            name: e.name.text().trim().to_string(),
            kind: ServerKind::Dimse,
            host: host.to_owned(),
            port,
            ae_title: ae.to_owned(),
            our_ae: our.to_owned(),
            tls: e.tls,
            retrieve: e.retrieve,
            receive_address: address.to_owned(),
            receive_port,
            ..Server::default()
        }))
    }
}

fn retrieve_text(r: Retrieve) -> String {
    match r {
        Retrieve::Get => t!("pacs-retrieve-get"),
        Retrieve::Move => t!("pacs-retrieve-move"),
    }
}

fn kind_text(k: ServerKind) -> String {
    match k {
        ServerKind::DicomWeb => t!("dicomweb-kind-web"),
        ServerKind::Dimse => t!("dicomweb-kind-dimse"),
    }
}

/// Builds what the sheet draws.
fn build_view(d: &Dialog, servers: &ServerList, persistent: bool) -> DialogView {
    let server = d.server.and_then(|i| servers.servers.get(i));
    let busy = d.job.is_some();
    let slots = d.slots();
    let mut form = Vec::new();
    for slot in &slots {
        let (label, control) = match *slot {
            Slot::Field(id) => {
                let f = d.field(id);
                let text = f.map(|f| f.text().to_string()).unwrap_or_default();
                let sel = f.map_or(0..0, |f| f.selection());
                let (label, hint, masked) = match id {
                    FieldId::Name => (t!("dicomweb-field-name"), t!("dicomweb-hint-name"), false),
                    FieldId::Url => (t!("dicomweb-field-url"), t!("dicomweb-hint-url"), false),
                    FieldId::User => (t!("dicomweb-field-user"), String::new(), false),
                    FieldId::Secret => {
                        let bearer = d.edit.as_ref().is_some_and(|e| e.auth == AuthKind::Bearer);
                        let kept = d.edit.as_ref().is_some_and(|e| e.has_secret);
                        (
                            if bearer {
                                t!("dicomweb-field-token")
                            } else {
                                t!("dicomweb-field-password")
                            },
                            if kept {
                                t!("dicomweb-hint-secret-kept")
                            } else {
                                String::new()
                            },
                            true,
                        )
                    }
                    FieldId::PatientName => {
                        (t!("dicomweb-field-patient-name"), String::new(), false)
                    }
                    FieldId::PatientId => (t!("dicomweb-field-patient-id"), String::new(), false),
                    FieldId::DateFrom => (
                        t!("dicomweb-field-date-from"),
                        t!("dicomweb-hint-date"),
                        false,
                    ),
                    FieldId::DateTo => (
                        t!("dicomweb-field-date-to"),
                        t!("dicomweb-hint-date"),
                        false,
                    ),
                    FieldId::Modality => (
                        t!("dicomweb-field-modality"),
                        t!("dicomweb-hint-modality"),
                        false,
                    ),
                    FieldId::Accession => (t!("dicomweb-field-accession"), String::new(), false),
                    FieldId::Host => (t!("dicomweb-field-host"), t!("dicomweb-hint-host"), false),
                    FieldId::Port => (t!("dicomweb-field-port"), t!("dicomweb-hint-port"), false),
                    FieldId::AeTitle => (t!("dicomweb-field-ae"), t!("dicomweb-hint-ae"), false),
                    FieldId::OurAe => (
                        t!("dicomweb-field-our-ae"),
                        DEFAULT_OUR_AE.to_owned(),
                        false,
                    ),
                    FieldId::ReceiveAddress => (
                        t!("pacs-field-receive-address"),
                        t!("pacs-hint-receive-address"),
                        false,
                    ),
                    FieldId::ReceivePort => (
                        t!("pacs-field-receive-port"),
                        t!("pacs-hint-receive-port"),
                        false,
                    ),
                };
                (
                    label,
                    Control::Field {
                        id,
                        text,
                        caret: f.map_or(0, LineEdit::caret),
                        selection: (sel.start, sel.end),
                        masked,
                        hint,
                        preedit: d.preedit.clone(),
                    },
                )
            }
            Slot::Auth => (
                t!("dicomweb-field-auth"),
                Control::Choice {
                    value: auth_text(d.edit.as_ref().map_or(AuthKind::None, |e| e.auth)),
                },
            ),
            Slot::ShowPatient => (
                t!("dicomweb-show-patient"),
                Control::Switch { on: d.show_patient },
            ),
            Slot::Kind => (
                t!("dicomweb-field-kind"),
                Control::Choice {
                    value: kind_text(d.edit.as_ref().map_or(ServerKind::DicomWeb, |e| e.kind)),
                },
            ),
            Slot::Scope => (
                t!("pacs-what"),
                Control::Choice {
                    value: d
                        .send
                        .as_ref()
                        .map(|s| send::scope_text(s.choice()))
                        .unwrap_or_default(),
                },
            ),
            Slot::Anonymise => (
                t!("pacs-anonymise"),
                Control::Switch {
                    on: d.send.as_ref().is_some_and(|s| s.anonymise),
                },
            ),
            Slot::Tls => (
                t!("dicomweb-field-tls"),
                Control::Switch {
                    on: d.edit.as_ref().is_some_and(|e| e.tls),
                },
            ),
            Slot::Retrieve => (
                t!("pacs-field-retrieve"),
                Control::Choice {
                    value: retrieve_text(d.edit.as_ref().map_or(Retrieve::Get, |e| e.retrieve)),
                },
            ),
        };
        form.push(FormItem { label, control });
    }
    let rows = d.rows(servers);
    let list: Vec<ListEntry> = rows
        .iter()
        .map(|r| {
            let selected = d.selected == Some(*r);
            match *r {
                Row::Server(i) => {
                    let s = &servers.servers[i];
                    let mut detail = match s.kind {
                        ServerKind::DicomWeb => vec![s.base_url.clone(), auth_text(s.auth)],
                        ServerKind::Dimse => vec![s.address(), kind_text(s.kind)],
                    };
                    if !s.is_https() {
                        detail.push(t!("dicomweb-not-encrypted"));
                    }
                    ListEntry {
                        title: s.label(),
                        detail: specular_i18n::dot_join(&detail),
                        selected,
                        ..ListEntry::default()
                    }
                }
                Row::Study(i) => study_entry(
                    &d.search.studies[i],
                    d.show_patient,
                    d.search.open == Some(i),
                    selected,
                ),
                Row::Series(_, j) => series_entry(&d.search.series[j], selected),
                Row::MoreStudies => more_entry(0),
                Row::MoreSeries(_) => more_entry(1),
            }
        })
        .collect();
    let mut notes = Vec::new();
    let (title, subtitle, list_title, empty, buttons) = match d.page {
        Page::Servers
            if let Some(s) = d
                .removing
                .as_ref()
                .and_then(|id| servers.servers.iter().find(|s| &s.id == id)) =>
        {
            notes.push((t!("dicomweb-remove-ask", server = s.label()), Tone::Warning));
            notes.push((t!("dicomweb-remove-detail"), Tone::Plain));
            (
                t!("dicomweb-title"),
                None,
                None,
                None,
                vec![
                    button(Button::Confirm, t!("dicomweb-remove"), true, false),
                    button(Button::Cancel, t!("app-cancel"), true, true),
                ],
            )
        }
        Page::Servers => {
            if !persistent && !servers.servers.is_empty() {
                notes.push((t!("dicomweb-session-only"), Tone::Plain));
            }
            let sel = matches!(d.selected, Some(Row::Server(_)));
            // DICOMweb by QIDO-RS, DIMSE by C-FIND.
            let web = sel;
            (
                t!("dicomweb-title"),
                Some(t!("dicomweb-servers-intro")),
                Some(t!("dicomweb-servers")),
                Some(t!("dicomweb-servers-empty")),
                vec![
                    button(Button::Add, t!("dicomweb-add"), true, false),
                    button(Button::Edit, t!("dicomweb-edit"), sel, false),
                    button(Button::Remove, t!("dicomweb-remove"), sel, false),
                    button(Button::Close, t!("dicomweb-close"), true, false),
                    button(Button::Browse, t!("dicomweb-browse"), web, true),
                ],
            )
        }
        Page::Send => send_page(d, servers, &mut notes),
        Page::Edit if d.edit.as_ref().is_some_and(|e| e.kind == ServerKind::Dimse) => {
            let e = d.edit.as_ref();
            let check = d.dimse_check();
            if let (Some(Err(p)), Some(e)) = (&check, e)
                && e.tried
            {
                notes.push((p.clone(), Tone::Error));
            }
            notes.push((t!("dicomweb-dimse-note-open"), Tone::Plain));
            if let Some(e) = e.filter(|e| e.retrieve == Retrieve::Move) {
                let our = e.our_ae.text().trim();
                let ae = if our.is_empty() { DEFAULT_OUR_AE } else { our };
                notes.push((t!("pacs-move-note", ae = ae.to_owned()), Tone::Plain));
            }
            (
                if e.is_some_and(|e| e.id.is_some()) {
                    t!("dicomweb-edit-title")
                } else {
                    t!("dicomweb-add-title")
                },
                None,
                None,
                None,
                vec![
                    button(Button::Cancel, t!("app-cancel"), true, false),
                    button(Button::Verify, t!("dicomweb-verify"), !busy, false),
                    button(Button::Save, t!("dicomweb-save"), true, true),
                ],
            )
        }
        Page::Edit => {
            let e = d.edit.as_ref();
            let check = d.url_check();
            if let (Some(Err(p)), Some(e)) = (check, e)
                && (e.tried || !e.url.text().trim().is_empty())
                && (e.tried || p != UrlProblem::Empty)
            {
                notes.push((url_problem_text(p), Tone::Error));
            }
            if let (Some(Ok(u)), Some(e)) = (d.url_check(), e)
                && e.auth != AuthKind::None
                && u.starts_with("http://")
            {
                notes.push((t!("dicomweb-http-warning"), Tone::Warning));
            }
            if e.is_some_and(|e| e.auth != AuthKind::None) && !persistent {
                notes.push((t!("dicomweb-secret-session"), Tone::Plain));
            }
            let ok = matches!(d.url_check(), Some(Ok(_)));
            (
                if e.is_some_and(|e| e.id.is_some()) {
                    t!("dicomweb-edit-title")
                } else {
                    t!("dicomweb-add-title")
                },
                None,
                None,
                None,
                vec![
                    button(Button::Cancel, t!("app-cancel"), true, false),
                    button(Button::Verify, t!("dicomweb-verify"), ok && !busy, false),
                    button(Button::Save, t!("dicomweb-save"), ok, true),
                ],
            )
        }
        Page::Search => {
            let mut b = vec![
                button(Button::Back, t!("dicomweb-back"), true, false),
                button(Button::Close, t!("dicomweb-close"), true, false),
            ];
            if busy {
                b.push(button(Button::Stop, t!("dicomweb-stop"), true, false));
            }
            b.push(button(Button::Search, t!("dicomweb-search"), !busy, true));
            let searched = d.search.query.is_some();
            (
                t!("dicomweb-search-title"),
                server.map(|s| s.label()),
                Some(t!("dicomweb-studies")),
                Some(if searched {
                    t!("dicomweb-no-studies")
                } else {
                    t!("dicomweb-search-hint")
                }),
                b,
            )
        }
    };
    let status = if busy && d.page == Page::Edit {
        Some((t!("dicomweb-verifying"), Tone::Plain))
    } else if busy {
        Some((t!("dicomweb-searching"), Tone::Plain))
    } else if d.page == Page::Send {
        d.send
            .as_ref()
            .and_then(send_status)
            .or_else(|| d.message.clone())
    } else {
        d.message.clone()
    };
    let progress = d
        .send
        .as_ref()
        .filter(|s| d.page == Page::Send && s.running())
        .map(|s| s.done as f32 / s.total().max(1) as f32);
    DialogView {
        page: d.page,
        title,
        subtitle,
        form,
        list_title,
        list,
        empty,
        scroll: d.scroll,
        notes,
        status,
        progress,
        buttons,
        focus: d.focus,
    }
}

/// Most failures listed by name in the summary.
const LISTED_FAILURES: usize = 6;

/// The Send page: what, where, the question, progress, the summary.
#[allow(clippy::type_complexity)]
fn send_page(
    d: &Dialog,
    servers: &ServerList,
    notes: &mut Vec<(String, Tone)>,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<ButtonView>,
) {
    let Some(s) = d.send.as_ref() else {
        return (
            t!("pacs-title"),
            None,
            None,
            None,
            vec![button(Button::Close, t!("dicomweb-close"), true, true)],
        );
    };
    let c = s.choice();
    let target = d.server.and_then(|i| servers.servers.get(i));
    let summary = t!(
        "pacs-summary",
        studies = c.studies as i64,
        files = c.files.len() as i64
    );
    match s.stage {
        send::Stage::Choose => {
            notes.push((
                if s.anonymise {
                    t!("pacs-anonymise-note")
                } else {
                    t!("pacs-identified-note")
                },
                if s.anonymise {
                    Tone::Plain
                } else {
                    Tone::Warning
                },
            ));
            (
                t!("pacs-title"),
                Some(summary),
                Some(t!("pacs-to")),
                Some(t!("pacs-no-servers")),
                vec![
                    button(Button::Close, t!("app-cancel"), true, false),
                    button(
                        Button::Send,
                        t!("pacs-send"),
                        target.is_some() && !c.files.is_empty(),
                        true,
                    ),
                ],
            )
        }
        send::Stage::Confirm => {
            let server = target.map(Server::label).unwrap_or_default();
            notes.push((
                t!(
                    "pacs-confirm",
                    studies = c.studies as i64,
                    files = c.files.len() as i64,
                    server = server
                ),
                Tone::Warning,
            ));
            notes.push((
                if s.anonymise {
                    t!("pacs-anonymise-note")
                } else {
                    t!("pacs-identified-note")
                },
                Tone::Plain,
            ));
            (
                t!("pacs-title"),
                Some(summary),
                None,
                None,
                vec![
                    button(Button::Back, t!("pacs-go-back"), true, false),
                    button(Button::Confirm, t!("pacs-confirm-send"), true, true),
                ],
            )
        }
        send::Stage::Running { .. } => (
            t!("pacs-title"),
            Some(s.server.clone()),
            None,
            None,
            vec![button(Button::Stop, t!("dicomweb-stop"), true, true)],
        ),
        send::Stage::Done => {
            for f in s.failed.iter().take(LISTED_FAILURES) {
                notes.push((
                    t!(
                        "pacs-failed-line",
                        name = f.name.clone(),
                        reason = f.reason.clone()
                    ),
                    Tone::Error,
                ));
            }
            if s.failed.len() > LISTED_FAILURES {
                notes.push((
                    t!(
                        "pacs-failed-more",
                        count = (s.failed.len() - LISTED_FAILURES) as i64
                    ),
                    Tone::Error,
                ));
            }
            if s.warnings > 0 {
                notes.push((
                    t!("pacs-warnings", count = s.warnings as i64),
                    Tone::Warning,
                ));
            }
            (
                t!("pacs-title"),
                Some(s.server.clone()),
                None,
                None,
                vec![button(Button::Close, t!("dicomweb-close"), true, true)],
            )
        }
    }
}

/// The Send page's status line.
fn send_status(s: &send::Sending) -> Option<(String, Tone)> {
    let total = s.total() as i64;
    match s.stage {
        send::Stage::Running { .. } => Some((
            t!(
                "pacs-sending",
                done = s.done as i64,
                total = total,
                server = s.server.clone()
            ),
            Tone::Plain,
        )),
        send::Stage::Done => Some(if let Some(f) = &s.fatal {
            (
                t!(
                    "pacs-stopped-because",
                    sent = s.sent() as i64,
                    total = total,
                    reason = f.clone()
                ),
                Tone::Error,
            )
        } else if s.cancelled {
            (
                t!("pacs-stopped", sent = s.sent() as i64, total = total),
                Tone::Warning,
            )
        } else if s.failed.is_empty() {
            (
                t!("pacs-sent-all", total = total, server = s.server.clone()),
                Tone::Plain,
            )
        } else {
            (
                t!(
                    "pacs-sent-some",
                    sent = s.sent() as i64,
                    total = total,
                    server = s.server.clone()
                ),
                Tone::Warning,
            )
        }),
        _ => None,
    }
}

fn button(id: Button, label: String, enabled: bool, default: bool) -> ButtonView {
    ButtonView {
        id,
        label,
        enabled,
        default,
    }
}

impl DicomWebState {
    fn persistent(&self) -> bool {
        self.creds.persistent()
    }

    /// The sheet as the draw function sees it.
    pub(super) fn view(&self) -> Option<DialogView> {
        let d = self.dialog.as_ref()?;
        Some(build_view(d, &self.servers, self.persistent()))
    }

    /// Draws the sheet `v` (when open) over everything.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: Option<&DialogView>,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) {
        if let (Some(v), Some(d)) = (v, self.dialog.as_mut()) {
            d.drawn = view::draw(dl, text, pal, v, size, rtl, ts);
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &DialogView, scale: f64) {
        if let Some(d) = &self.dialog {
            view::a11y(tree, v, &d.drawn, scale);
        }
    }

    /// The focused field (for the input method's window).
    pub(super) fn ime_area(&self) -> Option<Rect> {
        let d = self.dialog.as_ref().filter(|_| self.wants_text())?;
        d.drawn.ime.or(d.drawn.field)
    }

    /// What the input method composes in the focused field (empty: nothing).
    pub(super) fn set_preedit(&mut self, s: &str) {
        let wants = self.wants_text();
        if let Some(d) = self.dialog.as_mut() {
            if wants {
                s.clone_into(&mut d.preedit);
            } else {
                d.preedit.clear();
            }
        }
    }

    /// Remove on the Servers page: the selected server is asked about; the question's text.
    fn ask_remove(&mut self) -> Option<String> {
        let d = self.dialog.as_mut()?;
        if d.page != Page::Servers || d.removing.is_some() {
            return None;
        }
        let Some(Row::Server(i)) = d.selected else {
            return None;
        };
        let s = self.servers.servers.get(i)?;
        d.removing = Some(s.id.clone());
        Some(t!("dicomweb-remove-ask", server = s.label()))
    }

    /// The question accepted: the server leaves the list and its credentials are forgotten.
    /// The next one (or the one before) is selected. The list is not saved here.
    fn confirm_remove(&mut self) -> Option<Server> {
        let d = self.dialog.as_mut()?;
        let id = d.removing.take()?;
        let i = self.servers.servers.iter().position(|s| s.id == id)?;
        let s = self.servers.remove(&id)?;
        self.creds.forget(&id);
        let n = self.servers.servers.len();
        let next = (n > 0).then(|| i.min(n - 1));
        d.server = next;
        d.selected = next.map(Row::Server);
        Some(s)
    }

    /// True while a text field of the sheet has the keyboard.
    pub(super) fn wants_text(&self) -> bool {
        self.view().is_some_and(|v| v.focused_field().is_some())
    }

    fn load_servers(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if self.servers_file.is_none() && self.demo.is_none() {
            self.servers_file = Some(
                specular_settings::Locations::detect()
                    .config_dir
                    .join("dicomweb.json"),
            );
        }
        if let Some(p) = &self.servers_file {
            self.servers = ServerList::load(p);
        }
    }

    fn save_servers(&self) -> Option<String> {
        let p = self.servers_file.as_ref()?;
        self.servers
            .save(p)
            .err()
            .map(|e| crate::error_text::reason(&e))
    }

    fn client(&mut self) -> Result<Client, String> {
        if let Some(c) = &self.client {
            return Ok(c.clone());
        }
        let c = Client::new(ClientConfig::default())
            .map_err(|e| crate::error_text::reason(&e))?
            .tracked(specular_core::net::What::Dicom);
        self.client = Some(c.clone());
        Ok(c)
    }

    /// The credentials to use with `s`; `None` when they are needed and not known.
    fn auth_for(&self, s: &Server) -> Option<Auth> {
        match s.auth {
            AuthKind::None => Some(Auth::None),
            _ => self.creds.load(&s.id),
        }
    }

    /// Who answers searches on server `i` (as [`Self::web`] for DICOMweb).
    fn finder(&mut self, i: usize) -> Result<Finder, Option<String>> {
        match self.servers.servers.get(i) {
            Some(s) if s.kind == ServerKind::Dimse => Ok(Finder::Dimse(s.clone())),
            _ => self.web(i).map(Finder::Web),
        }
    }

    /// A client for server `i`, or why there is none (credentials needed: `Err(None)`).
    fn web(&mut self, i: usize) -> Result<DicomWeb, Option<String>> {
        let s = self.servers.servers.get(i).cloned().ok_or(None)?;
        let auth = self.auth_for(&s).ok_or(None)?;
        let client = self.client().map_err(Some)?;
        Ok(DicomWeb::new(client, &s.base_url, auth))
    }
}

impl App {
    pub(super) fn dicomweb_view(&self) -> Option<DialogView> {
        self.dicomweb.view()
    }

    pub(super) fn dicomweb_open(&mut self) {
        if self.dicomweb.dialog.is_some() {
            return;
        }
        self.dicomweb.load_servers();
        let mut d = Dialog::new();
        if !self.dicomweb.servers.servers.is_empty() {
            d.server = Some(0);
            d.selected = Some(Row::Server(0));
        }
        self.dicomweb.dialog = Some(d);
        self.dicomweb_focus_default();
        self.chrome.announce_status(t!("dicomweb-title"));
        self.redraw = true;
    }

    /// Open address… with a DICOMweb address: the sheet, on the saved server with that
    /// address, else on a new server's form with the address filled in (named after its
    /// host).
    pub(super) fn dicomweb_open_address(&mut self, url: &str) {
        let Ok(base) = check_url(url) else {
            self.dicomweb_open();
            return;
        };
        self.dicomweb_open();
        let found = self
            .dicomweb
            .servers
            .servers
            .iter()
            .position(|s| s.base_url.trim_end_matches('/') == base);
        match found {
            Some(i) => {
                if let Some(d) = self.dicomweb.dialog.as_mut() {
                    d.server = Some(i);
                    d.selected = Some(Row::Server(i));
                }
                self.dicomweb_focus_default();
            }
            None => {
                self.dicomweb_edit(None);
                let host = base
                    .split_once("://")
                    .map_or(base.as_str(), |(_, r)| r)
                    .split(['/', ':'])
                    .next()
                    .unwrap_or("")
                    .to_owned();
                if let Some(e) = self.dicomweb.dialog.as_mut().and_then(|d| d.edit.as_mut()) {
                    e.url.set_text(&base);
                    e.name.set_text(&host);
                }
            }
        }
        self.redraw = true;
    }

    /// Whether the Servers page is asking about removing one.
    fn dicomweb_removing(&self) -> bool {
        self.dicomweb
            .dialog
            .as_ref()
            .is_some_and(|d| d.page == Page::Servers && d.removing.is_some())
    }

    fn dicomweb_close(&mut self) {
        if let Some(d) = self.dicomweb.dialog.take()
            && let Some(j) = d.job
        {
            j.cancel.cancel();
        }
        self.redraw = true;
    }

    /// Focus on the first field, else the selected row, else the default button.
    fn dicomweb_focus_default(&mut self) {
        let Some(v) = self.dicomweb_view() else {
            return;
        };
        let f = v.focusables();
        let selected = v.list.iter().position(|e| e.selected).map(Hit::Entry);
        let target = if !v.form.is_empty() {
            Some(Hit::Form(0))
        } else if let Some(s) = selected {
            Some(s)
        } else {
            v.buttons
                .iter()
                .find(|b| b.default && b.enabled)
                .or_else(|| v.buttons.iter().find(|b| b.enabled))
                .map(|b| Hit::Button(b.id))
        };
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.focus = target.and_then(|t| f.iter().position(|x| *x == t));
        }
    }

    fn dicomweb_set_focus(&mut self, h: Hit) {
        let Some(v) = self.dicomweb_view() else {
            return;
        };
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.focus = v.focusables().iter().position(|x| *x == h);
            if let Hit::Entry(k) = h {
                let vis = d.drawn.visible.max(1);
                if k < d.scroll {
                    d.scroll = k;
                } else if k >= d.scroll + vis {
                    d.scroll = k + 1 - vis;
                }
            }
        }
        self.redraw = true;
    }

    fn dicomweb_page(&mut self, page: Page) {
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.page = page;
            d.scroll = 0;
            d.message = None;
            d.removing = None;
            if page == Page::Servers {
                d.selected = d.server.map(Row::Server);
                d.edit = None;
            }
        }
        self.dicomweb_focus_default();
        if let Some(v) = self.dicomweb_view() {
            self.chrome.announce_status(v.title);
        }
        self.redraw = true;
    }

    fn dicomweb_edit(&mut self, existing: Option<usize>) {
        let server = existing.and_then(|i| self.dicomweb.servers.servers.get(i).cloned());
        let has_secret = server
            .as_ref()
            .is_some_and(|s| s.auth != AuthKind::None && self.dicomweb.creds.load(&s.id).is_some());
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return;
        };
        let s = server.unwrap_or_default();
        d.edit = Some(Edit {
            id: (!s.id.is_empty()).then(|| s.id.clone()),
            name: LineEdit::new(&s.name),
            url: LineEdit::new(&s.base_url),
            auth: s.auth,
            user: LineEdit::new(&s.user),
            secret: LineEdit::new(""),
            has_secret,
            tried: false,
            kind: s.kind,
            host: LineEdit::new(&s.host),
            port: LineEdit::new(&if s.port == 0 {
                DEFAULT_PORT.to_string()
            } else {
                s.port.to_string()
            }),
            ae: LineEdit::new(&s.ae_title),
            our_ae: LineEdit::new(&s.our_ae),
            tls: s.tls,
            retrieve: s.retrieve,
            receive_address: LineEdit::new(&s.receive_address),
            receive_port: LineEdit::new(&if s.receive_port == 0 {
                String::new()
            } else {
                s.receive_port.to_string()
            }),
        });
        d.page = Page::Edit;
        d.message = None;
        self.dicomweb_focus_default();
        self.chrome
            .announce_status(self.dicomweb_view().map(|v| v.title).unwrap_or_default());
        self.redraw = true;
    }

    fn dicomweb_save(&mut self) {
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return;
        };
        let Some(e) = d.edit.as_mut() else { return };
        e.tried = true;
        if e.kind == ServerKind::Dimse {
            match d.dimse_check() {
                Some(Ok(server)) => {
                    let id = self.dicomweb.servers.upsert(server);
                    self.dicomweb.creds.forget(&id);
                    self.dicomweb_saved(&id);
                }
                Some(Err(msg)) => {
                    self.chrome.announce_status(msg);
                    self.redraw = true;
                }
                None => {}
            }
            return;
        }
        let url = match check_url(e.url.text()) {
            Ok(u) => u,
            Err(p) => {
                let msg = url_problem_text(p);
                self.chrome.announce_status(msg);
                self.redraw = true;
                return;
            }
        };
        let server = Server {
            id: e.id.clone().unwrap_or_default(),
            name: e.name.text().trim().to_string(),
            base_url: url,
            auth: e.auth,
            user: e.user.text().trim().to_string(),
            ..Server::default()
        };
        let secret = e.secret.text().to_string();
        let kind = e.auth;
        let id = self.dicomweb.servers.upsert(server);
        match kind {
            AuthKind::None => self.dicomweb.creds.forget(&id),
            AuthKind::Basic | AuthKind::Bearer if !secret.is_empty() => {
                let s = &self.dicomweb.servers.servers;
                let user = s
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.user.clone())
                    .unwrap_or_default();
                let auth = if kind == AuthKind::Basic {
                    Auth::Basic {
                        user,
                        password: Secret::new(secret),
                    }
                } else {
                    Auth::Bearer(Secret::new(secret))
                };
                self.dicomweb.creds.store(&id, auth);
            }
            _ => {
                // Keep the password or token already given (the user name may have changed).
                if kind == AuthKind::Basic
                    && let Some(Auth::Basic { password, .. }) = self.dicomweb.creds.load(&id)
                {
                    let user = self
                        .dicomweb
                        .servers
                        .get(&id)
                        .map(|s| s.user.clone())
                        .unwrap_or_default();
                    self.dicomweb
                        .creds
                        .store(&id, Auth::Basic { user, password });
                }
            }
        }
        self.dicomweb_saved(&id);
    }

    /// After Save: the list written, the server selected, back to the list.
    fn dicomweb_saved(&mut self, id: &str) {
        if let Some(err) = self.dicomweb.save_servers() {
            let now = self.now();
            self.chrome
                .warning(t!("dicomweb-save-failed", reason = err), now);
        }
        let i = self
            .dicomweb
            .servers
            .servers
            .iter()
            .position(|s| s.id == id);
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.server = i;
            d.edit = None;
        }
        self.dicomweb_page(Page::Servers);
    }

    /// Remove: asks first, naming the server (nothing on disk is lost).
    fn dicomweb_remove_ask(&mut self) {
        let Some(ask) = self.dicomweb.ask_remove() else {
            return;
        };
        self.dicomweb_focus_default();
        self.chrome.announce_status(ask);
        self.redraw = true;
    }

    /// Cancel or Esc on the question: the server stays, selected as before.
    fn dicomweb_remove_cancel(&mut self) {
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.removing = None;
        }
        self.dicomweb_focus_default();
        self.redraw = true;
    }

    /// Remove confirmed: the server goes, with its credentials for this session.
    fn dicomweb_remove(&mut self) {
        let removed = self.dicomweb.confirm_remove();
        if let Some(s) = removed {
            if let Some(err) = self.dicomweb.save_servers() {
                let now = self.now();
                self.chrome
                    .warning(t!("dicomweb-save-failed", reason = err), now);
            }
            self.chrome
                .announce_status(t!("dicomweb-removed", server = s.label()));
        }
        self.dicomweb_focus_default();
        self.redraw = true;
    }

    /// Browse: the search page of the selected server (credentials first when needed).
    fn dicomweb_browse(&mut self) {
        let Some(d) = self.dicomweb.dialog.as_ref() else {
            return;
        };
        let Some(Row::Server(i)) = d.selected else {
            return;
        };
        let Some(s) = self.dicomweb.servers.servers.get(i).cloned() else {
            return;
        };
        if s.kind == ServerKind::DicomWeb && self.dicomweb.auth_for(&s).is_none() {
            self.dicomweb_edit(Some(i));
            if let Some(d) = self.dicomweb.dialog.as_mut() {
                d.message = Some((t!("dicomweb-secret-needed"), Tone::Warning));
                // Focus on the password or token.
                let slots = d.slots();
                let f = slots
                    .iter()
                    .position(|s| *s == Slot::Field(FieldId::Secret));
                d.focus = f;
            }
            return;
        }
        let d = self.dicomweb.dialog.as_mut().expect("open");
        if d.server != Some(i) {
            d.search = Search::default();
        }
        d.server = Some(i);
        d.show_patient = false;
        self.dicomweb_page(Page::Search);
    }

    /// Sends the search (`append`: the next page of the last one).
    fn dicomweb_search(&mut self, append: bool) {
        let Some(d) = self.dicomweb.dialog.as_ref() else {
            return;
        };
        if d.job.is_some() {
            return;
        }
        let Some(i) = d.server else { return };
        let query = if append {
            match &d.search.query {
                Some(q) => StudyQuery {
                    offset: d.search.studies.len() as u32,
                    ..q.clone()
                },
                None => return,
            }
        } else {
            d.query()
        };
        let web = match self.dicomweb.finder(i) {
            Ok(w) => w,
            Err(e) => {
                if let Some(d) = self.dicomweb.dialog.as_mut() {
                    d.message = Some((
                        e.unwrap_or_else(|| t!("dicomweb-secret-needed")),
                        Tone::Error,
                    ));
                }
                self.redraw = true;
                return;
            }
        };
        let label = self
            .dicomweb
            .servers
            .servers
            .get(i)
            .map(Server::label)
            .unwrap_or_default();
        let cancel = CancelToken::new();
        let c = cancel.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dcmweb-search", move |l| {
            let res = web.studies(&query, &label, &c);
            let _ = tx.send(Outcome::Studies { append, query, res });
            l.waker()();
        });
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.job = Some(Job { rx, cancel });
            d.message = None;
        }
        self.chrome.announce_status(t!("dicomweb-searching"));
        self.redraw = true;
    }

    /// Lists the series of study `i` (`append`: the next page).
    fn dicomweb_series(&mut self, i: usize, append: bool) {
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return;
        };
        if d.job.is_some() {
            return;
        }
        let Some(study) = d.search.studies.get(i).map(|s| s.study_uid.clone()) else {
            return;
        };
        if !append {
            d.search.open = Some(i);
            d.search.series.clear();
            d.search.series_more = false;
        }
        let offset = if append {
            d.search.series.len() as u32
        } else {
            0
        };
        let Some(si) = d.server else { return };
        let web = match self.dicomweb.finder(si) {
            Ok(w) => w,
            Err(_) => return,
        };
        let label = self
            .dicomweb
            .servers
            .servers
            .get(si)
            .map(Server::label)
            .unwrap_or_default();
        let cancel = CancelToken::new();
        let c = cancel.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dcmweb-series", move |l| {
            let res = web.series(&study, offset, &label, &c);
            let _ = tx.send(Outcome::Series { study, append, res });
            l.waker()();
        });
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.job = Some(Job { rx, cancel });
            d.message = None;
        }
        self.redraw = true;
    }

    fn dicomweb_stop(&mut self) {
        if let Some(d) = self.dicomweb.dialog.as_mut()
            && let Some(j) = d.job.take()
        {
            j.cancel.cancel();
            d.message = Some((t!("dicomweb-stopped"), Tone::Plain));
            self.chrome.announce_status(t!("dicomweb-stopped"));
        }
        self.dicomweb_focus_default();
        self.redraw = true;
    }

    fn dicomweb_finished(&mut self, out: Outcome) {
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return;
        };
        d.job = None;
        let announce;
        match out {
            Outcome::Verified { server, res } => {
                let (msg, tone) = match res {
                    Ok(()) => (t!("dicomweb-verify-ok", server = server), Tone::Plain),
                    Err(e) => (e, Tone::Error),
                };
                d.message = Some((msg.clone(), tone));
                announce = msg;
            }
            Outcome::Studies { append, query, res } => match res {
                Ok(page) => {
                    if !append {
                        d.search.studies.clear();
                        d.search.open = None;
                        d.search.series.clear();
                        d.scroll = 0;
                        d.selected = None;
                    }
                    let first_new = d.search.studies.len();
                    d.search.studies.extend(page.rows);
                    d.search.more = page.more;
                    d.search.query = Some(query);
                    d.message = None;
                    announce = if d.search.studies.is_empty() {
                        t!("dicomweb-no-studies")
                    } else {
                        t!("dicomweb-found", count = d.search.studies.len() as i64)
                    };
                    if append && first_new < d.search.studies.len() {
                        d.selected = Some(Row::Study(first_new));
                    }
                }
                Err(None) => announce = t!("dicomweb-stopped"),
                Err(Some(m)) => {
                    d.message = Some((m.clone(), Tone::Error));
                    announce = m;
                }
            },
            Outcome::Series { study, append, res } => {
                let open_uid = d
                    .search
                    .open
                    .and_then(|i| d.search.studies.get(i))
                    .map(|s| s.study_uid.as_str());
                if open_uid != Some(study.as_str()) {
                    return;
                }
                match res {
                    Ok(page) => {
                        if !append {
                            d.search.series.clear();
                        }
                        d.search.series.extend(page.rows);
                        d.search.series_more = page.more;
                        announce = t!(
                            "dicomweb-series-found",
                            count = d.search.series.len() as i64
                        );
                    }
                    Err(None) => announce = t!("dicomweb-stopped"),
                    Err(Some(m)) => {
                        d.message = Some((m.clone(), Tone::Error));
                        announce = m;
                    }
                }
            }
        }
        // Keep the focus on the selected row (rows moved), else where it was.
        let sel = d.selected;
        self.chrome.announce_status(announce);
        if let Some(sel) = sel
            && let Some(d) = self.dicomweb.dialog.as_ref()
            && let Some(k) = d
                .rows(&self.dicomweb.servers)
                .iter()
                .position(|r| *r == sel)
        {
            self.dicomweb_set_focus(Hit::Entry(k));
        } else if self.dicomweb_view().is_some_and(|v| v.focused().is_none()) {
            self.dicomweb_focus_default();
        }
        self.redraw = true;
    }

    /// The searches' answers and the series stream.
    pub(super) fn dicomweb_tick(&mut self) {
        self.dicomweb_stream_tick();
        self.dicomweb_send_tick();
        let Some(d) = self.dicomweb.dialog.as_ref() else {
            return;
        };
        let Some(j) = &d.job else { return };
        match j.rx.try_recv() {
            Ok(out) => self.dicomweb_finished(out),
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                if let Some(d) = self.dicomweb.dialog.as_mut() {
                    d.job = None;
                }
                self.redraw = true;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
        }
    }

    /// A list row chosen (Enter, or a click on the selected row).
    fn dicomweb_activate(&mut self, row: Row) {
        let sending = self
            .dicomweb
            .dialog
            .as_ref()
            .is_some_and(|d| d.page == Page::Send);
        match row {
            Row::Server(_) if sending => self.dicomweb_send_ask(),
            Row::Server(_) => self.dicomweb_browse(),
            Row::Study(i) => {
                let open = self.dicomweb.dialog.as_ref().and_then(|d| d.search.open);
                if open == Some(i) {
                    if let Some(d) = self.dicomweb.dialog.as_mut() {
                        d.search.open = None;
                        d.search.series.clear();
                    }
                } else {
                    self.dicomweb_series(i, false);
                }
            }
            Row::Series(i, j) => self.dicomweb_open_series(i, j),
            Row::MoreStudies => self.dicomweb_search(true),
            Row::MoreSeries(i) => self.dicomweb_series(i, true),
        }
        self.redraw = true;
    }

    fn dicomweb_open_series(&mut self, i: usize, j: usize) {
        let Some(d) = self.dicomweb.dialog.as_ref() else {
            return;
        };
        let (Some(study), Some(series), Some(si)) =
            (d.search.studies.get(i), d.search.series.get(j), d.server)
        else {
            return;
        };
        let (study, series) = (study.study_uid.clone(), series.clone());
        let Some(server) = self.dicomweb.servers.servers.get(si).cloned() else {
            return;
        };
        if server.kind == ServerKind::Dimse {
            let label = series_entry(&series, false).title;
            match self.dicomweb_retrieve_start(&server, &study, &series.series_uid, label) {
                Ok(true) => self.dicomweb_close(),
                Ok(false) => {}
                Err(m) => {
                    if let Some(d) = self.dicomweb.dialog.as_mut() {
                        d.message = Some((m.clone(), Tone::Warning));
                    }
                    self.chrome.announce_status(m);
                    self.redraw = true;
                }
            }
            return;
        }
        let web = match self.dicomweb.web(si) {
            Ok(w) => w,
            Err(_) => return,
        };
        let label = series_entry(&series, false).title;
        let web = web.tracked(specular_core::net::What::DicomSeries(label.clone()));
        if self.dicomweb_stream_start(web, &server, &study, &series.series_uid, label) {
            self.dicomweb_close();
        }
    }

    fn dicomweb_select(&mut self, k: usize) {
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return;
        };
        let rows = d.rows(&self.dicomweb.servers);
        let Some(&row) = rows.get(k) else { return };
        let again = d.selected == Some(row);
        if matches!(row, Row::MoreStudies | Row::MoreSeries(_)) {
            self.dicomweb_activate(row);
            return;
        }
        d.selected = Some(row);
        if let Row::Server(i) = row {
            d.server = Some(i);
        }
        self.dicomweb_set_focus(Hit::Entry(k));
        if again || matches!(row, Row::Study(_)) {
            self.dicomweb_activate(row);
        }
    }

    fn dicomweb_step_auth(&mut self, delta: i32) {
        if let Some(d) = self.dicomweb.dialog.as_mut()
            && let Some(e) = d.edit.as_mut()
        {
            const ALL: [AuthKind; 3] = [AuthKind::None, AuthKind::Basic, AuthKind::Bearer];
            let i = ALL.iter().position(|k| *k == e.auth).unwrap_or(0) as i32;
            e.auth = ALL[(i + delta).rem_euclid(3) as usize];
            self.chrome.announce_status(auth_text(e.auth));
        }
        self.redraw = true;
    }

    fn dicomweb_toggle_tls(&mut self) {
        if let Some(e) = self.dicomweb.dialog.as_mut().and_then(|d| d.edit.as_mut()) {
            e.tls = !e.tls;
            // The well-known ports follow, unless another was typed (PS3.15 B.1: 2762).
            let (from, to) = if e.tls {
                (DEFAULT_PORT, DEFAULT_TLS_PORT)
            } else {
                (DEFAULT_TLS_PORT, DEFAULT_PORT)
            };
            if e.port.text().trim() == from.to_string() {
                e.port.set_text(&to.to_string());
            }
            self.chrome.announce_status(if e.tls {
                t!("dicomweb-tls-on")
            } else {
                t!("dicomweb-tls-off")
            });
        }
        self.redraw = true;
    }

    fn dicomweb_step_retrieve(&mut self) {
        if let Some(e) = self.dicomweb.dialog.as_mut().and_then(|d| d.edit.as_mut()) {
            e.retrieve = match e.retrieve {
                Retrieve::Get => Retrieve::Move,
                Retrieve::Move => Retrieve::Get,
            };
            self.chrome.announce_status(retrieve_text(e.retrieve));
        }
        self.redraw = true;
    }

    fn dicomweb_toggle_patient(&mut self) {
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            d.show_patient = !d.show_patient;
            self.chrome.announce_status(if d.show_patient {
                t!("dicomweb-patient-shown")
            } else {
                t!("dicomweb-patient-hidden")
            });
        }
        self.redraw = true;
    }

    fn dicomweb_hit(&mut self, h: Hit) {
        match h {
            Hit::Form(i) => {
                let slot = self
                    .dicomweb
                    .dialog
                    .as_ref()
                    .and_then(|d| d.slots().get(i).copied());
                self.dicomweb_set_focus(h);
                match slot {
                    Some(Slot::Auth) => self.dicomweb_step_auth(1),
                    Some(Slot::ShowPatient) => self.dicomweb_toggle_patient(),
                    Some(Slot::Kind) => self.dicomweb_step_kind(),
                    Some(Slot::Scope) => self.dicomweb_step_scope(1),
                    Some(Slot::Anonymise) => self.dicomweb_toggle_anonymise(),
                    Some(Slot::Tls) => self.dicomweb_toggle_tls(),
                    Some(Slot::Retrieve) => self.dicomweb_step_retrieve(),
                    _ => {}
                }
            }
            Hit::Entry(k) => self.dicomweb_select(k),
            Hit::Button(b) => match b {
                Button::Back
                    if self
                        .dicomweb
                        .dialog
                        .as_ref()
                        .is_some_and(|d| d.page == Page::Send) =>
                {
                    self.dicomweb_send_back()
                }
                Button::Stop
                    if self
                        .dicomweb
                        .dialog
                        .as_ref()
                        .is_some_and(|d| d.page == Page::Send) =>
                {
                    self.dicomweb_send_stop()
                }
                Button::Verify => self.dicomweb_verify(),
                Button::Send => self.dicomweb_send_ask(),
                Button::Confirm if self.dicomweb_removing() => self.dicomweb_remove(),
                Button::Confirm => self.dicomweb_send_start(),
                Button::Close => self.dicomweb_close(),
                Button::Add => self.dicomweb_edit(None),
                Button::Edit => {
                    let sel = self.dicomweb.dialog.as_ref().and_then(|d| d.selected);
                    if let Some(Row::Server(i)) = sel {
                        self.dicomweb_edit(Some(i));
                    }
                }
                Button::Remove => self.dicomweb_remove_ask(),
                Button::Browse => self.dicomweb_browse(),
                Button::Cancel if self.dicomweb_removing() => self.dicomweb_remove_cancel(),
                Button::Cancel => self.dicomweb_page(Page::Servers),
                Button::Save => self.dicomweb_save(),
                Button::Back => {
                    self.dicomweb_stop_job();
                    self.dicomweb_page(Page::Servers);
                }
                Button::Search => self.dicomweb_search(false),
                Button::Stop => self.dicomweb_stop(),
            },
        }
        self.redraw = true;
    }

    fn dicomweb_stop_job(&mut self) {
        if let Some(d) = self.dicomweb.dialog.as_mut()
            && let Some(j) = d.job.take()
        {
            j.cancel.cancel();
        }
    }

    /// Enter: the focused button or row, else the page's default.
    fn dicomweb_enter(&mut self, v: &DialogView) {
        match v.focused() {
            Some(h @ (Hit::Button(_) | Hit::Entry(_))) => {
                if let Hit::Entry(k) = h {
                    let row = self
                        .dicomweb
                        .dialog
                        .as_ref()
                        .and_then(|d| d.rows(&self.dicomweb.servers).get(k).copied());
                    if let (Some(row), Some(d)) = (row, self.dicomweb.dialog.as_mut()) {
                        d.selected = Some(row);
                        if let Row::Server(i) = row {
                            d.server = Some(i);
                        }
                        self.dicomweb_activate(row);
                    }
                } else {
                    self.dicomweb_hit(h);
                }
            }
            _ => {
                if let Some(b) = v.buttons.iter().find(|b| b.default && b.enabled) {
                    self.dicomweb_hit(Hit::Button(b.id));
                }
            }
        }
    }

    /// Editing keys in the focused field; false when the key is not one.
    fn dicomweb_field_key(
        &mut self,
        v: &DialogView,
        combo: Option<&KeyCombo>,
        text: Option<&str>,
    ) -> bool {
        let rtl = self
            .dicomweb
            .dialog
            .as_ref()
            .and_then(|d| d.drawn.field_text.as_ref())
            .is_some_and(|(b, _)| b.rtl);
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return false;
        };
        let Some(field) = d.focused_field_mut(v) else {
            return false;
        };
        if let Some(c) = combo {
            let mut clip = PendingClipboard::default();
            let handled = field.key(c.key, c.mods, rtl, &mut clip).used();
            if handled {
                self.field_clipboard(clip, PasteTo::DicomWeb);
                self.redraw = true;
                return true;
            }
            if c.mods.ctrl || c.mods.alt || c.mods.meta {
                return false;
            }
        }
        let Some(t) = text else { return false };
        let s: String = t.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return false;
        }
        self.last_key_text = Some((s.clone(), Instant::now()));
        self.dicomweb_text(&s);
        true
    }

    /// Text into the focused field (typed keys and input-method commits).
    pub(super) fn dicomweb_text(&mut self, s: &str) {
        let Some(v) = self.dicomweb_view() else {
            return;
        };
        if let Some(d) = self.dicomweb.dialog.as_mut()
            && let Some(f) = d.focused_field_mut(&v)
            && f.insert(s)
        {
            self.redraw = true;
        }
    }

    /// Text pasted into the focused field.
    pub(super) fn dicomweb_paste(&mut self, s: &str) {
        let Some(v) = self.dicomweb_view() else {
            return;
        };
        if let Some(d) = self.dicomweb.dialog.as_mut()
            && let Some(f) = d.focused_field_mut(&v)
        {
            f.paste(s);
            self.redraw = true;
        }
    }

    /// Keys while the sheet is open (it takes them all); `text` is what the key types.
    pub(super) fn dicomweb_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) -> bool {
        let Some(v) = self.dicomweb_view() else {
            return false;
        };
        let focus = v.focused();
        if v.focused_field().is_some() && self.dicomweb_field_key(&v, combo, text) {
            return true;
        }
        let Some(combo) = combo else { return true };
        let focusables = v.focusables();
        let n = focusables.len().max(1) as i32;
        let move_focus = |me: &mut App, d: i32| {
            let i = v.focus.map_or(0, |i| (i as i32 + d).rem_euclid(n) as usize);
            if let Some(h) = focusables.get(i).copied() {
                me.dicomweb_set_focus(h);
            }
        };
        let rtl = self.rtl();
        let plain = combo.mods == Mods::NONE;
        let slot = |me: &App, i: usize| {
            me.dicomweb
                .dialog
                .as_ref()
                .and_then(|d| d.slots().get(i).copied())
        };
        match combo.key {
            Key::Named(NamedKey::Escape) if v.page == Page::Send => {
                let stage = self
                    .dicomweb
                    .dialog
                    .as_ref()
                    .and_then(|d| d.send.as_ref())
                    .map(|s| match s.stage {
                        send::Stage::Confirm => 1,
                        send::Stage::Running { .. } => 2,
                        _ => 0,
                    });
                match stage {
                    Some(1) => self.dicomweb_send_back(),
                    Some(2) => self.dicomweb_send_stop(),
                    _ => self.dicomweb_close(),
                }
            }
            Key::Named(NamedKey::Escape) if self.dicomweb_removing() => {
                self.dicomweb_remove_cancel()
            }
            Key::Named(NamedKey::Escape) => {
                let busy = self
                    .dicomweb
                    .dialog
                    .as_ref()
                    .is_some_and(|d| d.job.is_some());
                if busy {
                    self.dicomweb_stop();
                } else if v.page == Page::Edit {
                    self.dicomweb_page(Page::Servers);
                } else {
                    self.dicomweb_close();
                }
            }
            Key::Named(NamedKey::Tab) if combo.mods.shift => move_focus(self, -1),
            Key::Named(NamedKey::Tab) => move_focus(self, 1),
            Key::Named(NamedKey::ArrowUp) if plain => move_focus(self, -1),
            Key::Named(NamedKey::ArrowDown) if plain => move_focus(self, 1),
            Key::Named(NamedKey::Home) if plain && matches!(focus, Some(Hit::Entry(_))) => {
                self.dicomweb_set_focus(Hit::Entry(0));
            }
            Key::Named(NamedKey::End) if plain && matches!(focus, Some(Hit::Entry(_))) => {
                self.dicomweb_set_focus(Hit::Entry(v.list.len().saturating_sub(1)));
            }
            Key::Named(k @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if plain => {
                let forward = (k == NamedKey::ArrowRight) != rtl;
                match focus {
                    Some(Hit::Form(i)) if slot(self, i) == Some(Slot::Auth) => {
                        self.dicomweb_step_auth(if forward { 1 } else { -1 })
                    }
                    Some(Hit::Form(i)) if slot(self, i) == Some(Slot::Kind) => {
                        self.dicomweb_step_kind()
                    }
                    Some(Hit::Form(i)) if slot(self, i) == Some(Slot::Retrieve) => {
                        self.dicomweb_step_retrieve()
                    }
                    Some(Hit::Form(i)) if slot(self, i) == Some(Slot::Scope) => {
                        self.dicomweb_step_scope(if forward { 1 } else { -1 })
                    }
                    // A study opens forward and closes backward, as in a tree.
                    Some(Hit::Entry(k)) => {
                        let row = self
                            .dicomweb
                            .dialog
                            .as_ref()
                            .and_then(|d| d.rows(&self.dicomweb.servers).get(k).copied());
                        let open = self.dicomweb.dialog.as_ref().and_then(|d| d.search.open);
                        match row {
                            Some(Row::Study(i)) if forward != (open == Some(i)) => {
                                if let Some(d) = self.dicomweb.dialog.as_mut() {
                                    d.selected = row;
                                }
                                self.dicomweb_activate(Row::Study(i));
                            }
                            Some(Row::Series(i, _)) if !forward => {
                                let rows = self
                                    .dicomweb
                                    .dialog
                                    .as_ref()
                                    .map(|d| d.rows(&self.dicomweb.servers))
                                    .unwrap_or_default();
                                if let Some(p) = rows.iter().position(|r| *r == Row::Study(i)) {
                                    if let Some(d) = self.dicomweb.dialog.as_mut() {
                                        d.selected = Some(Row::Study(i));
                                    }
                                    self.dicomweb_set_focus(Hit::Entry(p));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => move_focus(self, if forward { 1 } else { -1 }),
                }
            }
            Key::Named(NamedKey::Space) => match focus {
                Some(Hit::Form(i)) => match slot(self, i) {
                    Some(Slot::Auth) => self.dicomweb_step_auth(1),
                    Some(Slot::ShowPatient) => self.dicomweb_toggle_patient(),
                    Some(Slot::Kind) => self.dicomweb_step_kind(),
                    Some(Slot::Scope) => self.dicomweb_step_scope(1),
                    Some(Slot::Anonymise) => self.dicomweb_toggle_anonymise(),
                    Some(Slot::Tls) => self.dicomweb_toggle_tls(),
                    Some(Slot::Retrieve) => self.dicomweb_step_retrieve(),
                    _ => {}
                },
                Some(h) => self.dicomweb_hit(h),
                None => {}
            },
            Key::Named(NamedKey::Enter) => self.dicomweb_enter(&v),
            Key::Named(NamedKey::Delete) if plain && v.page == Page::Servers => {
                self.dicomweb_remove_ask()
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// A click while the sheet is open (it takes them all).
    pub(super) fn dicomweb_click(&mut self, p: (f32, f32), down: bool) -> bool {
        let (now, shift) = (self.now(), self.mods.shift_key());
        let Some(v) = self.dicomweb_view() else {
            return false;
        };
        let Some(d) = self.dicomweb.dialog.as_mut() else {
            return false;
        };
        if !down {
            if let Some(f) = d.focused_field_mut(&v) {
                f.release();
            }
            return true;
        }
        let hit = d
            .drawn
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|x| x.1);
        let Some(h) = hit else { return true };
        // A press in the focused field moves its caret.
        if v.focused() == Some(h)
            && let Some(id) = v.focused_field()
            && let Some((block, x0)) = d.drawn.field_text.clone()
            && id != FieldId::Secret
            && let Some(f) = d.field_mut(id)
        {
            f.press(block.hit_byte(p.0 - x0), p.0, now, shift);
            self.redraw = true;
            return true;
        }
        self.dicomweb_hit(h);
        true
    }

    /// The pointer moved with the button down: a selection in the focused field.
    pub(super) fn dicomweb_drag(&mut self, p: (f32, f32)) {
        let Some(v) = self.dicomweb_view() else {
            return;
        };
        if let Some(d) = self.dicomweb.dialog.as_mut()
            && let Some((block, x0)) = d.drawn.field_text.clone()
            && let Some(f) = d.focused_field_mut(&v)
            && f.dragging()
        {
            f.drag_to(block.hit_byte(p.0 - x0));
            self.redraw = true;
        }
    }

    /// The wheel scrolls the list.
    pub(super) fn dicomweb_wheel(&mut self, lines: f32) {
        let Some(v) = self.dicomweb_view() else {
            return;
        };
        if let Some(d) = self.dicomweb.dialog.as_mut() {
            let max = v.list.len().saturating_sub(d.drawn.visible);
            let step = (-lines).round() as i64;
            d.scroll = (d.scroll as i64 + step).clamp(0, max as i64) as usize;
            self.redraw = true;
        }
    }

    /// A screen reader's request on one of the sheet's nodes; false when not ours.
    pub(super) fn dicomweb_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.dicomweb_view() else {
            return false;
        };
        let Some(hit) = view::a11y_target(&v, req.target_node.0) else {
            return (A11Y_BASE..A11Y_BASE + 0x10_0000).contains(&req.target_node.0);
        };
        match (req.action, hit) {
            (accesskit::Action::Increment, Hit::Form(_)) => self.dicomweb_step_auth(1),
            (accesskit::Action::Decrement, Hit::Form(_)) => self.dicomweb_step_auth(-1),
            (accesskit::Action::SetValue, Hit::Form(i)) => {
                if let Some(accesskit::ActionData::Value(s)) = &req.data
                    && let Some(d) = self.dicomweb.dialog.as_mut()
                    && let Some(Slot::Field(id)) = d.slots().get(i).copied()
                    && let Some(f) = d.field_mut(id)
                {
                    f.set_text(s);
                }
            }
            (accesskit::Action::SetTextSelection, Hit::Form(i)) => {
                if let Some(accesskit::ActionData::SetTextSelection(sel)) = &req.data
                    && let Some(d) = self.dicomweb.dialog.as_mut()
                    && let Some(Slot::Field(id)) = d.slots().get(i).copied()
                    && let Some(f) = d.field_mut(id)
                {
                    f.set_a11y_selection(sel.anchor.character_index, sel.focus.character_index);
                }
                self.dicomweb_set_focus(hit);
            }
            (accesskit::Action::Click, h) => self.dicomweb_hit(h),
            (accesskit::Action::Focus | accesskit::Action::ScrollIntoView, h) => {
                self.dicomweb_set_focus(h)
            }
            _ => {}
        }
        self.redraw = true;
        true
    }

    /// `--show dicomweb`, `dicomweb-edit`, `dicomweb-search`, `dicomweb-series`: the sheet
    /// against a synthetic server on loopback (nothing read from or written to the config).
    /// False when `scene` is not one of these.
    pub(super) fn dicomweb_show(&mut self, scene: &str) -> bool {
        if !matches!(
            scene,
            "dicomweb" | "dicomweb-edit" | "dicomweb-search" | "dicomweb-series"
        ) {
            return false;
        }
        let fake = match specular_dicomweb::fake::FakeServer::start(
            specular_dicomweb::fake::FakeConfig::synthetic(8, 24, 128),
            specular_remote::testing::ServerOptions::default(),
        ) {
            Ok(f) => f,
            Err(e) => {
                specular_log::warn!(target: specular_log::UI, "dicomweb demo server: {e}");
                return true;
            }
        };
        let st = &mut self.dicomweb;
        st.loaded = true;
        st.servers_file = None;
        st.servers = demo_servers(&fake.base());
        st.demo = Some(fake);
        self.dicomweb_open();
        match scene {
            "dicomweb-edit" => self.dicomweb_edit(Some(1)),
            "dicomweb-search" | "dicomweb-series" => {
                self.dicomweb_browse();
                if let Some(d) = self.dicomweb.dialog.as_mut() {
                    d.search.modality.set_text("CT");
                }
                // Synchronously: loopback, and the shot waits for nothing else.
                if let Ok(web) = self.dicomweb.web(0)
                    && let Some(d) = self.dicomweb.dialog.as_ref()
                {
                    let query = d.query();
                    let res = web
                        .search_studies(&query, &CancelToken::new())
                        .map_err(|e| Some(error_text(&e, "demo")));
                    let series = res.as_ref().ok().and_then(|p| p.rows.first()).map(|s| {
                        web.search_series(&s.study_uid, PAGE_SIZE, 0, &CancelToken::new())
                            .map_err(|e| Some(error_text(&e, "demo")))
                    });
                    self.dicomweb_finished(Outcome::Studies {
                        append: false,
                        query,
                        res,
                    });
                    if scene == "dicomweb-series"
                        && let Some(res) = series
                        && let Some(d) = self.dicomweb.dialog.as_mut()
                    {
                        d.search.open = Some(0);
                        d.selected = Some(Row::Series(0, 0));
                        let study = d.search.studies[0].study_uid.clone();
                        self.dicomweb_finished(Outcome::Series {
                            study,
                            append: false,
                            res,
                        });
                    }
                }
            }
            _ => {}
        }
        true
    }
}

/// The servers the demo shows: the synthetic server, and two that only look real.
fn demo_servers(base: &str) -> ServerList {
    let mut l = ServerList::default();
    l.upsert(Server {
        name: "Synthetic test server".into(),
        base_url: base.to_string(),
        ..Server::default()
    });
    l.upsert(Server {
        name: "Teaching archive".into(),
        base_url: "https://pacs.example.org/dicom-web".into(),
        auth: AuthKind::Basic,
        user: "reader".into(),
        ..Server::default()
    });
    l.upsert(Server {
        name: String::new(),
        base_url: "https://imaging.example.net/wado".into(),
        auth: AuthKind::Bearer,
        ..Server::default()
    });
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Remove asks first, naming the server; Esc or Cancel keeps it; the answer removes it
    /// and forgets its sign-in.
    #[test]
    fn remove_asks_before_the_server_goes() {
        let mut st = DicomWebState {
            servers: demo_servers("http://127.0.0.1:8042/dicom-web"),
            loaded: true,
            ..DicomWebState::default()
        };
        let id = st.servers.servers[1].id.clone();
        st.creds.store(
            &id,
            Auth::Basic {
                user: "reader".into(),
                password: Secret::new("hunter2"),
            },
        );
        let mut d = Dialog::new();
        d.server = Some(1);
        d.selected = Some(Row::Server(1));
        st.dialog = Some(d);
        let ask = st.ask_remove().expect("asks");
        assert!(ask.contains("Teaching archive"), "{ask}");
        // Nothing is gone yet; the page shows only the question.
        assert_eq!(st.servers.servers.len(), 3);
        let v = st.view().unwrap();
        assert!(v.list.is_empty());
        assert!(v.notes.iter().any(|(n, _)| n.contains("Teaching archive")));
        let ids: Vec<Button> = v.buttons.iter().map(|b| b.id).collect();
        assert_eq!(ids, [Button::Confirm, Button::Cancel]);
        assert!(
            v.buttons
                .iter()
                .any(|b| b.id == Button::Cancel && b.default)
        );
        assert!(st.ask_remove().is_none(), "asked once");
        // Cancel (as Esc does).
        st.dialog.as_mut().unwrap().removing = None;
        assert_eq!(st.servers.servers.len(), 3);
        assert!(st.creds.load(&id).is_some());
        assert_eq!(st.view().unwrap().list.len(), 3);
        // Asked again and accepted.
        st.ask_remove().unwrap();
        let gone = st.confirm_remove().expect("removed");
        assert_eq!(gone.id, id);
        assert_eq!(st.servers.servers.len(), 2);
        assert!(st.servers.servers.iter().all(|s| s.id != id));
        assert!(st.creds.load(&id).is_none(), "credentials forgotten");
        let d = st.dialog.as_ref().unwrap();
        assert_eq!(d.selected, Some(Row::Server(1)));
        assert!(d.removing.is_none());
        assert!(st.confirm_remove().is_none(), "nothing asked");
    }

    #[test]
    fn names_and_dates_read_well() {
        assert_eq!(person_text("FAKE^ALPHA"), "FAKE, ALPHA");
        assert_eq!(person_text("FAKE^ALPHA^B^^"), "FAKE, ALPHA B");
        assert_eq!(person_text("SOLO"), "SOLO");
        assert_eq!(person_text("^GIVEN"), "GIVEN");
        assert_eq!(date_text("20250214"), "2025-02-14");
        assert_eq!(date_text("2025"), "2025");
    }

    #[test]
    fn patient_data_stays_hidden_until_asked() {
        let s = StudyRow {
            study_uid: "2.25.1".into(),
            date: "20250214".into(),
            description: "CT CHEST".into(),
            accession: "ACC123".into(),
            modalities: vec!["CT".into()],
            series_count: Some(2),
            instance_count: Some(120),
            patient: specular_dicomweb::Patient {
                name: "FAKE^ALPHA".into(),
                id: "PID-1".into(),
                ..Default::default()
            },
            ..StudyRow::default()
        };
        let hidden = study_entry(&s, false, false, false);
        for secret in ["FAKE", "PID-1", "ACC123"] {
            assert!(!hidden.title.contains(secret) && !hidden.detail.contains(secret));
        }
        let shown = study_entry(&s, true, false, false);
        for secret in ["FAKE", "PID-1", "ACC123"] {
            assert!(shown.detail.contains(secret), "{}", shown.detail);
        }
        assert!(shown.title.contains("2025-02-14") && shown.title.contains("CT CHEST"));
    }

    #[test]
    fn the_form_follows_the_kind_of_sign_in() {
        let mut d = Dialog::new();
        d.page = Page::Edit;
        d.edit = Some(Edit {
            id: None,
            name: LineEdit::new(""),
            url: LineEdit::new("ftp://x"),
            auth: AuthKind::None,
            user: LineEdit::new(""),
            secret: LineEdit::new(""),
            has_secret: false,
            tried: true,
            kind: ServerKind::DicomWeb,
            host: LineEdit::new(""),
            port: LineEdit::new(""),
            ae: LineEdit::new(""),
            our_ae: LineEdit::new(""),
            tls: false,
            retrieve: Retrieve::Get,
            receive_address: LineEdit::new(""),
            receive_port: LineEdit::new(""),
        });
        assert_eq!(d.slots().len(), 4);
        let v = build_view(&d, &ServerList::default(), false);
        assert!(v.notes.iter().any(|(_, t)| *t == Tone::Error));
        assert!(!v.buttons.iter().any(|b| b.id == Button::Save && b.enabled));
        let e = d.edit.as_mut().unwrap();
        e.auth = AuthKind::Basic;
        e.url.set_text("http://pacs.local/dicom-web");
        assert_eq!(d.slots().len(), 6);
        let v = build_view(&d, &ServerList::default(), false);
        assert!(
            v.notes.iter().any(|(_, t)| *t == Tone::Warning),
            "{:?}",
            v.notes
        );
        assert!(v.buttons.iter().any(|b| b.id == Button::Save && b.enabled));
        d.edit.as_mut().unwrap().auth = AuthKind::Bearer;
        assert_eq!(d.slots().len(), 5);
        // DIMSE: host, port and AE titles instead; a bad AE title is named once tried.
        let e = d.edit.as_mut().unwrap();
        e.kind = ServerKind::Dimse;
        e.host.set_text("pacs.local");
        e.port.set_text("11112");
        e.ae.set_text("BAD\\AE");
        assert_eq!(d.slots().len(), 8);
        let v = build_view(&d, &ServerList::default(), false);
        assert!(
            v.notes
                .iter()
                .any(|(n, t)| *t == Tone::Error && *n == t!("dicomweb-problem-ae"))
        );
        d.edit.as_mut().unwrap().ae.set_text("ARCHIVE");
        let s = d.dimse_check().unwrap().unwrap();
        assert_eq!(
            (s.host.as_str(), s.port, s.calling_ae()),
            ("pacs.local", 11112, "SPECULAR")
        );
        d.edit.as_mut().unwrap().port.set_text("70000");
        assert_eq!(d.dimse_check(), Some(Err(t!("dicomweb-problem-port"))));
        // C-MOVE: an address of this computer and a port to receive on, and a note
        // naming our AE title.
        let e = d.edit.as_mut().unwrap();
        e.port.set_text("11112");
        e.retrieve = Retrieve::Move;
        assert_eq!(d.slots().len(), 10);
        assert_eq!(
            d.dimse_check(),
            Some(Err(t!("pacs-problem-receive-address")))
        );
        d.edit
            .as_mut()
            .unwrap()
            .receive_address
            .set_text("192.168.1.20");
        assert_eq!(d.dimse_check(), Some(Err(t!("pacs-problem-receive-port"))));
        d.edit.as_mut().unwrap().receive_port.set_text("11113");
        let s = d.dimse_check().unwrap().unwrap();
        assert_eq!(
            (s.retrieve, s.receive_address.as_str(), s.receive_port),
            (Retrieve::Move, "192.168.1.20", 11113)
        );
        let v = build_view(&d, &ServerList::default(), false);
        assert!(
            v.notes
                .iter()
                .any(|(n, _)| *n == t!("pacs-move-note", ae = "SPECULAR"))
        );
        // Back to C-GET: the move fields go and are not needed.
        let e = d.edit.as_mut().unwrap();
        e.retrieve = Retrieve::Get;
        e.receive_port.set_text("");
        assert_eq!(d.slots().len(), 8);
        assert!(matches!(d.dimse_check(), Some(Ok(_))));
    }

    #[test]
    fn retrieval_failures_read_well() {
        use specular_dicomweb::retrieve::Failure;
        let s = Server {
            kind: ServerKind::Dimse,
            host: "pacs.local".into(),
            port: 11112,
            ae_title: "ARCHIVE".into(),
            receive_port: 11113,
            ..Server::default()
        };
        let mv = Server {
            retrieve: Retrieve::Move,
            ..s.clone()
        };
        let plain = |t: String| t.replace(['\u{2068}', '\u{2069}'], "");
        let text = |f: Failure, s: &Server| plain(stream::retrieve_text(&f, s));
        let no_ctx = || Failure::Dimse(specular_dimse::Error::NothingAccepted);
        assert!(text(no_ctx(), &s).contains("C-GET"));
        assert!(text(no_ctx(), &mv).contains("C-MOVE"));
        let t = text(Failure::Status(0xA801), &mv);
        assert!(t.contains("SPECULAR") && t.contains("11113"), "{t}");
        assert!(text(Failure::NoMatches, &s).contains("no images"));
        let roles = Failure::NothingArrived {
            failed: 3,
            roles_refused: true,
            moved: false,
        };
        assert!(text(roles, &s).contains("C-MOVE"));
        let moved = Failure::NothingArrived {
            failed: 3,
            roles_refused: false,
            moved: true,
        };
        assert!(text(moved, &mv).contains("firewall"));
        let t = text(Failure::Status(0xC001), &s);
        assert!(t.contains("C001"), "{t}");
        let t = plain(send::dimse_fatal(
            &specular_dimse::Error::Listen(std::io::Error::from(std::io::ErrorKind::AddrInUse)),
            &Server {
                receive_address: "192.168.1.20".into(),
                ..mv
            },
        ));
        assert!(t.contains("192.168.1.20") && t.contains("11113"), "{t}");
    }

    #[test]
    fn studies_list_their_series_under_them() {
        let mut d = Dialog::new();
        d.page = Page::Search;
        d.search.studies = vec![StudyRow::default(), StudyRow::default()];
        d.search.more = true;
        d.search.open = Some(0);
        d.search.series = vec![SeriesRow::default(); 2];
        let rows = d.rows(&ServerList::default());
        assert_eq!(
            rows,
            vec![
                Row::Study(0),
                Row::Series(0, 0),
                Row::Series(0, 1),
                Row::Study(1),
                Row::MoreStudies
            ]
        );
    }

    #[test]
    fn dimse_servers_are_searched_by_c_find() {
        use specular_dimse::scp::{ScpConfig, TestScp};
        let rec = |uid: &str, level: &str| {
            vec![
                (0x0008_0052, level.to_string()),
                (0x0020_000D, "1.2.1".to_string()),
                (0x0020_000E, uid.to_string()),
                (0x0008_0060, "CT".to_string()),
                (0x0008_0061, "CT".to_string()),
                (0x0010_0010, "FAKE^ALPHA".to_string()),
            ]
        };
        let scp = TestScp::start(ScpConfig {
            records: vec![rec("", "STUDY"), rec("1.2.1.1", "SERIES")],
            ..ScpConfig::default()
        })
        .unwrap();
        let server = Server {
            kind: ServerKind::Dimse,
            host: "127.0.0.1".into(),
            port: scp.port,
            ae_title: "TESTSCP".into(),
            ..Server::default()
        };
        let f = Finder::Dimse(server.clone());
        let c = CancelToken::new();
        let q = StudyQuery {
            modality: "CT".into(),
            limit: PAGE_SIZE,
            ..StudyQuery::default()
        };
        let p = f.studies(&q, "x", &c).unwrap();
        assert_eq!(p.rows.len(), 1);
        assert_eq!(p.rows[0].patient.name, "FAKE^ALPHA");
        let se = f.series("1.2.1", 0, "x", &c).unwrap();
        assert_eq!(se.rows[0].series_uid, "1.2.1.1");
        // A wrong AE title: the plain-language rejection.
        let wrong = Finder::Dimse(Server {
            ae_title: "OTHER".into(),
            ..server
        });
        let e = wrong.studies(&q, "x", &c).unwrap_err().unwrap();
        assert!(e.contains("OTHER"), "{e}");
        // A server that does not answer searches.
        let store_only = TestScp::start(ScpConfig {
            sop_classes: vec!["1.2.840.10008.5.1.4.1.1.2".into()],
            ..ScpConfig::default()
        })
        .unwrap();
        let none = Finder::Dimse(Server {
            kind: ServerKind::Dimse,
            host: "127.0.0.1".into(),
            port: store_only.port,
            ae_title: "TESTSCP".into(),
            ..Server::default()
        });
        assert_eq!(
            none.studies(&q, "x", &c).unwrap_err(),
            Some(t!("pacs-find-not-supported", host = "127.0.0.1"))
        );
    }
}
