use crate::bitstream::{Bitstream, U};
use crate::common::{Bundle, define_bundle};
use crate::grid::{AlignedGrid, AllocTracker, MutableSubgrid};
use crate::threadpool::JxlThreadPool;

use super::{
    ModularChannelInfo,
    predictor::{Predictor, WpHeader},
};
use crate::modular::{Error, Result, Sample, image::TransformedGrid};

mod palette;
mod rct;
pub(crate) mod squeeze;

#[derive(Debug, Clone)]
pub enum TransformInfo {
    Rct(Rct),
    Palette(Palette),
    Squeeze(Squeeze),
}

impl TransformInfo {
    pub(super) fn prepare_transform_info(
        &mut self,
        channels: &mut super::ModularChannels,
    ) -> Result<()> {
        match self {
            Self::Rct(rct) => rct.transform_channel_info(channels),
            Self::Palette(pal) => {
                pal.transform_channel_info::<i16>(channels, &mut Vec::new(), None)
            }
            Self::Squeeze(sq) => {
                sq.set_default_params(channels);
                sq.transform_channel_info::<i16>(channels, None, None)
            }
        }
    }

    pub(super) fn prepare_meta_channels<S: Sample>(
        &self,
        meta_channels: &mut Vec<AlignedGrid<S>>,
        tracker: Option<&AllocTracker>,
    ) -> Result<()> {
        if let Self::Palette(pal) = self {
            meta_channels.insert(
                0,
                AlignedGrid::with_alloc_tracker(
                    pal.nb_colours as usize,
                    pal.num_c as usize,
                    tracker,
                )?,
            );
        }
        Ok(())
    }

    /// Runs the channel-info part of this transform and records the size of every squeeze
    /// residual channel it creates, in creation order.
    ///
    /// Upstream stores squeeze residuals in the right/bottom half of the channel being squeezed
    /// and merges the two views back with raw pointers. This fork keeps residuals in their own
    /// storage instead, allocated from these sizes.
    pub(super) fn residual_sizes(
        &self,
        channels: &mut super::ModularChannels,
        out: &mut Vec<(usize, usize)>,
    ) -> Result<()> {
        match self {
            Self::Rct(rct) => rct.transform_channel_info(channels),
            Self::Palette(pal) => {
                pal.transform_channel_info::<i16>(channels, &mut Vec::new(), None)
            }
            Self::Squeeze(sq) => sq.transform_channel_info::<i16>(channels, None, Some(out)),
        }
    }

    /// `residuals` holds the views for the squeeze residual channels, in *reverse* creation
    /// order (they are popped).
    pub(super) fn transform_channels<'dest, S: Sample>(
        &self,
        channels: &mut super::ModularChannels,
        meta_channel_grids: &mut Vec<MutableSubgrid<'dest, S>>,
        grids: &mut Vec<TransformedGrid<'dest, S>>,
        residuals: &mut Vec<MutableSubgrid<'dest, S>>,
    ) -> Result<()> {
        match self {
            Self::Rct(rct) => rct.transform_channel_info(channels),
            Self::Palette(pal) => {
                pal.transform_channel_info(channels, meta_channel_grids, Some(grids))
            }
            Self::Squeeze(sq) => {
                sq.transform_channel_info(channels, Some((grids, residuals)), None)
            }
        }
    }

    pub(super) fn inverse<S: Sample>(
        &self,
        grids: &mut Vec<TransformedGrid<'_, S>>,
        bit_depth: u32,
        pool: &JxlThreadPool,
    ) {
        match self {
            Self::Rct(rct) => rct.inverse(grids, pool),
            Self::Palette(pal) => pal.inverse(grids, bit_depth),
            Self::Squeeze(sq) => sq.inverse(grids, pool),
        }
    }

    pub fn is_palette(&self) -> bool {
        matches!(self, Self::Palette(_))
    }

    pub fn is_squeeze(&self) -> bool {
        matches!(self, Self::Squeeze(_))
    }
}

impl Bundle<&WpHeader> for TransformInfo {
    type Error = crate::modular::Error;

    fn parse(bitstream: &mut Bitstream, wp_header: &WpHeader) -> crate::modular::Result<Self> {
        let tr = bitstream.read_bits(2)?;
        match tr {
            0 => Rct::parse(bitstream, ()).map(Self::Rct),
            1 => Palette::parse(bitstream, wp_header).map(Self::Palette),
            2 => Squeeze::parse(bitstream, ()).map(Self::Squeeze),
            value => Err(crate::modular::Error::Bitstream(
                crate::bitstream::Error::InvalidEnum {
                    name: "TransformId",
                    value,
                },
            )),
        }
    }
}

define_bundle! {
    #[derive(Debug, Clone)]
    pub struct Rct error(crate::modular::Error) {
        begin_c: ty(U32(u(3), 8 + u(6), 72 + u(10), 1096 + u(13))),
        rct_type: ty(U32(6, u(2), 2 + u(4), 10 + u(6))),
    }

    #[derive(Debug, Clone)]
    pub struct Squeeze error(crate::modular::Error) {
        num_sq: ty(U32(0, 1 + u(4), 9 + u(6), 41 + u(8))),
        sp: ty(Vec[Bundle(SqueezeParams)]; num_sq),
    }

    #[derive(Debug, Clone)]
    struct SqueezeParams error(crate::modular::Error) {
        horizontal: ty(Bool),
        in_place: ty(Bool),
        begin_c: ty(U32(u(3), 8 + u(6), 72 + u(10), 1096 + u(13))),
        num_c: ty(U32(1, 2, 3, 4 + u(4))),
    }
}

#[derive(Debug, Clone)]
pub struct Palette {
    begin_c: u32,
    num_c: u32,
    nb_colours: u32,
    nb_deltas: u32,
    d_pred: Predictor,
    wp_header: Option<WpHeader>,
}

impl Bundle<&WpHeader> for Palette {
    type Error = crate::modular::Error;

    fn parse(bitstream: &mut Bitstream, wp_header: &WpHeader) -> Result<Self> {
        let begin_c = bitstream.read_u32(U(3), 8 + U(6), 72 + U(10), 1096 + U(13))?;
        let num_c = bitstream.read_u32(1, 3, 4, 1 + U(13))?;
        let nb_colours = bitstream.read_u32(U(8), 256 + U(10), 1280 + U(12), 5376 + U(16))?;
        let nb_deltas = bitstream.read_u32(0, 1 + U(8), 257 + U(10), 1281 + U(16))?;
        let d_pred = Predictor::try_from(bitstream.read_bits(4)?)?;
        Ok(Self {
            begin_c,
            num_c,
            nb_colours,
            nb_deltas,
            d_pred,
            wp_header: if d_pred == Predictor::SelfCorrecting {
                Some(wp_header.clone())
            } else {
                None
            },
        })
    }
}

impl Rct {
    fn transform_channel_info(&self, channels: &mut super::ModularChannels) -> Result<()> {
        let begin_c = self.begin_c;
        let end_c = self.begin_c + 3;
        if end_c as usize > channels.info.len() {
            return Err(Error::InvalidRctParams);
        }

        let ModularChannelInfo { width, height, .. } = channels.info[begin_c as usize];
        for info in &channels.info[(begin_c + 1) as usize..end_c as usize] {
            if width != info.width || height != info.height {
                return Err(Error::InvalidRctParams);
            }
        }

        Ok(())
    }

    fn inverse<S: Sample>(&self, grids: &mut [TransformedGrid<'_, S>], pool: &JxlThreadPool) {
        let permutation = self.rct_type / 7;
        let ty = self.rct_type % 7;

        let begin_c = self.begin_c as usize;
        let mut channels = grids.iter_mut().skip(begin_c);
        let a = channels.next().unwrap().grid_mut();
        let b = channels.next().unwrap().grid_mut();
        let c = channels.next().unwrap().grid_mut();
        let grids = [a, b, c];

        match ty {
            0 => rct::inverse_rct::<_, 0>(permutation, grids, pool),
            1 => rct::inverse_rct::<_, 1>(permutation, grids, pool),
            2 => rct::inverse_rct::<_, 2>(permutation, grids, pool),
            3 => rct::inverse_rct::<_, 3>(permutation, grids, pool),
            4 => rct::inverse_rct::<_, 4>(permutation, grids, pool),
            5 => rct::inverse_rct::<_, 5>(permutation, grids, pool),
            6 => rct::inverse_rct::<_, 6>(permutation, grids, pool),
            _ => unreachable!(),
        }
    }
}

impl Palette {
    fn transform_channel_info<'dest, S: Sample>(
        &self,
        channels: &mut super::ModularChannels,
        meta_channel_grids: &mut Vec<MutableSubgrid<'dest, S>>,
        grids: Option<&mut Vec<TransformedGrid<'dest, S>>>,
    ) -> Result<()> {
        let begin_c = self.begin_c;
        let end_c = begin_c + self.num_c;
        if end_c as usize > channels.info.len() {
            return Err(Error::InvalidPaletteParams);
        }
        if begin_c < channels.nb_meta_channels {
            if end_c > channels.nb_meta_channels {
                return Err(Error::InvalidPaletteParams);
            }
            channels.nb_meta_channels = channels.nb_meta_channels + 2 - self.num_c;
        } else {
            channels.nb_meta_channels += 1;
        }

        let ModularChannelInfo { width, height, .. } = channels.info[begin_c as usize];
        for info in &channels.info[(begin_c as usize + 1)..end_c as usize] {
            if info.width != width || info.height != height {
                return Err(Error::InvalidPaletteParams);
            }
        }

        channels
            .info
            .drain((begin_c as usize + 1)..(end_c as usize));
        channels.info.insert(
            0,
            ModularChannelInfo::new_unshiftable(self.nb_colours, self.num_c),
        );

        if let Some(grids) = grids {
            let members = grids
                .drain((begin_c as usize + 1)..(end_c as usize))
                .collect::<Vec<_>>();
            grids[begin_c as usize].merge(members);

            let palette_grid = meta_channel_grids.pop().unwrap();
            grids.insert(0, TransformedGrid::from(palette_grid));
        }
        Ok(())
    }

    fn inverse<S: Sample>(&self, grids: &mut Vec<TransformedGrid<'_, S>>, bit_depth: u32) {
        let begin_c = self.begin_c as usize;
        let num_c = self.num_c as usize;

        let palette_grid = grids.remove(0);
        let leader = &mut grids[begin_c];
        let mut members = leader.unmerge(num_c - 1);
        let leader = leader.grid_mut();

        let palette = palette_grid.grid().as_shared();
        let mut targets = Vec::with_capacity(num_c);
        targets.push(leader.borrow_mut());
        for member in &mut members {
            targets.push(member.grid_mut().borrow_mut());
        }

        self.inverse_inner(palette, targets, bit_depth);

        for (i, grid) in members.into_iter().enumerate() {
            grids.insert(begin_c + 1 + i, grid);
        }
    }
}

impl Squeeze {
    fn set_default_params(&mut self, channels: &super::ModularChannels) {
        if !self.sp.is_empty() {
            return;
        }

        let first = channels.nb_meta_channels;
        let first_ch = &channels.info[first as usize];
        let mut w = first_ch.width;
        let mut h = first_ch.height;
        if channels.info.len() as u32 - first >= 3 {
            let next_ch = &channels.info[first as usize + 1];
            if next_ch.width == w && next_ch.height == h {
                let param_base = SqueezeParams {
                    begin_c: first + 1,
                    num_c: 2,
                    in_place: false,
                    horizontal: false,
                };
                self.sp.push(SqueezeParams {
                    horizontal: true,
                    ..param_base
                });
                self.sp.push(param_base);
            }
        }

        let param_base = SqueezeParams {
            begin_c: first,
            num_c: channels.info.len() as u32 - first,
            in_place: true,
            horizontal: false,
        };

        if h >= w && h > 8 {
            self.sp.push(SqueezeParams {
                horizontal: false,
                ..param_base
            });
            h = h.div_ceil(2);
        }
        while w > 8 || h > 8 {
            if w > 8 {
                self.sp.push(SqueezeParams {
                    horizontal: true,
                    ..param_base
                });
                w = w.div_ceil(2);
            }
            if h > 8 {
                self.sp.push(SqueezeParams {
                    horizontal: false,
                    ..param_base
                });
                h = h.div_ceil(2);
            }
        }
    }

    #[allow(clippy::type_complexity)]
    fn transform_channel_info<'dest, S: Sample>(
        &self,
        channels: &mut super::ModularChannels,
        mut grids: Option<(
            &mut Vec<TransformedGrid<'dest, S>>,
            &mut Vec<MutableSubgrid<'dest, S>>,
        )>,
        mut sizes: Option<&mut Vec<(usize, usize)>>,
    ) -> Result<()> {
        for sp in &self.sp {
            let SqueezeParams {
                horizontal,
                in_place,
                begin_c: begin,
                num_c,
            } = *sp;
            let end = begin + num_c;

            if end as usize > channels.info.len() {
                return Err(Error::InvalidSqueezeParams);
            }
            if begin < channels.nb_meta_channels {
                if !in_place || end > channels.nb_meta_channels {
                    return Err(Error::InvalidSqueezeParams);
                }
                channels.nb_meta_channels += num_c;
            }

            let mut residu_cap = num_c as usize;
            if in_place {
                residu_cap += channels.info.len() - end as usize;
            }
            let mut residu_channels = Vec::with_capacity(residu_cap);
            let mut residu_grids: Option<Vec<TransformedGrid<'dest, S>>> =
                grids.is_some().then(|| Vec::with_capacity(residu_cap));

            for (idx, ch) in channels.info[(begin as usize)..(end as usize)]
                .iter_mut()
                .enumerate()
            {
                let mut residu = ch.clone();
                let super::ModularChannelInfo {
                    width: w,
                    height: h,
                    hshift,
                    vshift,
                    ..
                } = ch;
                if *w == 0 || *h == 0 {
                    crate::tracing::error!(?ch, "Cannot squeeze zero-sized channel");
                    return Err(Error::InvalidSqueezeParams);
                }
                if *hshift > 30 || *vshift > 30 {
                    crate::tracing::error!(?ch, "Channel is sqeezed too much");
                    return Err(Error::InvalidSqueezeParams);
                }

                let (target_len, target_shift, residu_len, residu_shift) = if horizontal {
                    (w, hshift, &mut residu.width, &mut residu.hshift)
                } else {
                    (h, vshift, &mut residu.height, &mut residu.vshift)
                };
                let len = *target_len;
                *target_len = len.div_ceil(2);
                *residu_len = len / 2;
                if *target_shift >= 0 {
                    *target_shift += 1;
                    *residu_shift += 1;
                }

                let residu_size = (residu.width as usize, residu.height as usize);
                residu_channels.push(residu);
                if let Some(sizes) = &mut sizes {
                    sizes.push(residu_size);
                }

                if let Some(residu_grids) = &mut residu_grids {
                    let (grids, residuals) = grids.as_mut().unwrap();
                    let g = &mut grids[begin as usize + idx];
                    let g = g.grid_mut();
                    if horizontal {
                        g.shrink_width(g.width().div_ceil(2));
                    } else {
                        g.shrink_height(g.height().div_ceil(2));
                    }
                    let residu_grid = residuals.pop().ok_or(Error::InvalidSqueezeParams)?;
                    if (residu_grid.width(), residu_grid.height()) != residu_size {
                        return Err(Error::InvalidSqueezeParams);
                    }
                    residu_grids.push(TransformedGrid::from(residu_grid));
                }
            }

            if in_place {
                residu_channels.extend(channels.info.drain((end as usize)..));
            }
            channels.info.extend(residu_channels);

            if let Some((grids, _)) = &mut grids {
                let mut residu_grids = residu_grids.unwrap();
                if in_place {
                    residu_grids.extend(grids.drain((end as usize)..));
                }
                grids.extend(residu_grids);
            }
        }
        Ok(())
    }

    fn inverse<S: Sample>(&self, grids: &mut Vec<TransformedGrid<'_, S>>, pool: &JxlThreadPool) {
        for sp in self.sp.iter().rev() {
            let begin = sp.begin_c as usize;
            let channel_count = sp.num_c as usize;
            let end = begin + channel_count;
            let residual_channels: Vec<_> = if sp.in_place {
                grids.drain(end..(end + channel_count)).collect()
            } else {
                grids.drain((grids.len() - channel_count)..).collect()
            };

            for (ch, residu) in grids[begin..end].iter_mut().zip(residual_channels) {
                sp.inverse(ch, residu, pool);
            }
        }
    }
}

impl SqueezeParams {
    fn inverse<'dest, S: Sample>(
        &self,
        i0: &mut TransformedGrid<'dest, S>,
        i1: TransformedGrid<'dest, S>,
        pool: &crate::threadpool::JxlThreadPool,
    ) {
        let i0 = i0.grid_mut();
        let TransformedGrid::Single(i1) = i1 else {
            panic!("residual channel should be Single channel")
        };
        let residu = i1.as_shared();
        if self.horizontal {
            let width = i0.width() + residu.width();
            let height = i0.height();
            i0.regrow(width, height);
            if height > 16 {
                let groups = i0.borrow_mut().into_groups(width, 16);
                pool.for_each_vec(
                    groups.into_iter().enumerate().collect(),
                    |(idx, mut group)| {
                        let y0 = idx * 16;
                        let r = residu.subgrid(.., y0..y0 + group.height());
                        squeeze::inverse_h(&mut group, r)
                    },
                );
            } else {
                squeeze::inverse_h(i0, residu);
            }
        } else {
            let width = i0.width();
            let height = i0.height() + residu.height();
            i0.regrow(width, height);
            const COLS: usize = 256;
            if width > COLS {
                let groups = i0.borrow_mut().into_groups(COLS, height);
                pool.for_each_vec(
                    groups.into_iter().enumerate().collect(),
                    |(idx, mut group)| {
                        let x0 = idx * COLS;
                        let r = residu.subgrid(x0..x0 + group.width(), ..);
                        squeeze::inverse_v(&mut group, r)
                    },
                );
            } else {
                squeeze::inverse_v(i0, residu);
            }
        }
    }
}
