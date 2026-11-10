//! Small binary CGM files for other crates' tests (not a public API).

use crate::writer::{P, W};

/// Two pictures, white backgrounds:
///
/// 1. "metric": 1 mm per VDC unit, extent 100×50 VDC → 283.46×141.73 pt. A solid red
///    (255, 0, 0) square from VDC (10, 10) to (40, 40), i.e. page points x 28.35–113.39,
///    y 28.35–113.39 (y down), and the text "Specular" (height 5 mm) at VDC (50, 25).
/// 2. "abstract": extent 200×100 VDC, no metric scale → 792×396 pt. A black line
///    corner to corner and nothing else.
pub fn two_pictures() -> Vec<u8> {
    let mut w = W::begin("fixture");
    w.el(
        1,
        2,
        P::new().s("\"ProfileId:WebCGM\" \"ProfileEd:2.0\" \"Source:Specular tests\""),
    );
    w.el(1, 13, P::new().s("Helvetica"));
    // Picture 1: metric.
    w.el(0, 3, P::new().s("metric"));
    w.el(2, 1, P::new().e(1).r(1.0));
    w.el(2, 6, P::new().pt(0, 0).pt(100, 50));
    w.el(0, 4, P::new());
    w.el(5, 22, P::new().e(1));
    w.el(5, 23, P::new().u8(2).align());
    w.el(4, 7, P::new().pt(10, 10).pt(40, 10).pt(40, 40).pt(10, 40));
    w.el(5, 10, P::new().i(1));
    w.el(5, 15, P::new().i(5));
    w.el(4, 4, P::new().pt(50, 25).e(1).s("Specular"));
    w.end_picture();
    // Picture 2: abstract.
    w.picture("abstract", [0, 0, 200, 100]);
    w.el(4, 1, P::new().pt(0, 0).pt(200, 100));
    w.end_picture();
    w.finish()
}
