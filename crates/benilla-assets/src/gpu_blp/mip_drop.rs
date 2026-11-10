//! `WOW_MIP_DROP`: the decoded (no-BC) upload path drops the top levels of a wide chain, which is
//! where an RGBA8 fallback spends its VRAM (4-8x a block chain's). A carry of this repository's
//! for the Android port, kept out of `gpu_blp.rs` but for the one call in `for_upload`
//! (docs/UPSTREAM.md, "Android carries").

use std::sync::OnceLock;

use benilla_formats::BlpMipChain;

/// `WOW_MIP_DROP=n`: how many top levels the decoded path drops from a chain wider than
/// [`MIP_DROP_MIN_WIDTH`], read once like `WOW_NO_BC`. Android defaults to 1, since the RGBA8
/// fallback is the memory gate there and the top-down camera never sees texel detail at 1:1;
/// elsewhere 0, so a desktop `WOW_NO_BC=1 WOW_MIP_DROP=1` run is the A/B.
pub(super) fn mip_drop() -> u32 {
    static MIP_DROP: OnceLock<u32> = OnceLock::new();
    *MIP_DROP.get_or_init(|| {
        std::env::var("WOW_MIP_DROP")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_MIP_DROP)
    })
}

#[cfg(target_os = "android")]
const DEFAULT_MIP_DROP: u32 = 1;
#[cfg(not(target_os = "android"))]
const DEFAULT_MIP_DROP: u32 = 0;

/// Only chains wider than this are eligible for the drop: 1.12's 256-wide terrain and skin
/// textures stay whole, the 1024-wide building and foliage sheets are the ones that cost.
const MIP_DROP_MIN_WIDTH: u32 = 512;

/// The narrowest a dropped chain's new level 0 may be.
const MIP_DROP_FLOOR_WIDTH: u32 = 256;

/// How many top levels to drop from a `width`-wide chain of `levels` authored levels, asked for
/// `drop`: none unless wider than [`MIP_DROP_MIN_WIDTH`], never below [`MIP_DROP_FLOOR_WIDTH`],
/// always leaving a level.
fn levels_to_drop(width: u32, levels: usize, drop: u32) -> usize {
    if width <= MIP_DROP_MIN_WIDTH || levels == 0 {
        return 0;
    }
    let mut n = 0usize;
    while n < drop as usize && n + 1 < levels && (width >> (n + 1)) >= MIP_DROP_FLOOR_WIDTH {
        n += 1;
    }
    n
}

/// `chain` with its top levels removed per [`levels_to_drop`]; the stored levels are reused as
/// they are, so a block chain stays blocks until `into_rgba8`.
pub(super) fn drop_top_mips(mut chain: BlpMipChain, drop: u32) -> BlpMipChain {
    let n = levels_to_drop(chain.width, chain.mips.len(), drop);
    if n == 0 {
        return chain;
    }
    let (w, h) = chain.mip_size(n as u32);
    chain.mips.drain(..n);
    chain.width = w;
    chain.height = h;
    chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use benilla_formats::BlpTexels;

    /// A chain with every level down to 1×1, as the 1.12 BLPs author them.
    fn full_chain(texels: BlpTexels, w: u32, h: u32) -> BlpMipChain {
        let levels = 32 - w.max(h).leading_zeros();
        let mips = (0..levels)
            .map(|i| vec![0u8; texels.level_bytes((w >> i).max(1), (h >> i).max(1))])
            .collect();
        BlpMipChain {
            width: w,
            height: h,
            texels,
            mips,
        }
    }

    #[test]
    fn mip_drop_math() {
        // Nothing at or under 512 wide, whatever is asked.
        assert_eq!(levels_to_drop(512, 10, 2), 0);
        assert_eq!(levels_to_drop(256, 9, 1), 0);
        // Zero asked is zero dropped.
        assert_eq!(levels_to_drop(1024, 11, 0), 0);
        // 1024 → 512 at one, 1024 → 256 at two, and the 256 floor holds at three.
        assert_eq!(levels_to_drop(1024, 11, 1), 1);
        assert_eq!(levels_to_drop(1024, 11, 2), 2);
        assert_eq!(levels_to_drop(1024, 11, 3), 2);
        // 2048 → 256 is three; 9 asked still stops at the floor.
        assert_eq!(levels_to_drop(2048, 12, 9), 3);
        // A chain authored without mips has nothing to drop; one spare level allows one drop.
        assert_eq!(levels_to_drop(1024, 1, 1), 0);
        assert_eq!(levels_to_drop(1024, 2, 2), 1);
        assert_eq!(levels_to_drop(1024, 0, 2), 0);
    }

    #[test]
    fn dropped_chain_keeps_dimensions_and_bytes_in_step() {
        for texels in [BlpTexels::Bc1, BlpTexels::Bc3, BlpTexels::Rgba8Unorm] {
            let before = full_chain(texels, 1024, 512);
            let levels = before.mips.len();
            let c = drop_top_mips(before, 1);
            assert_eq!((c.width, c.height), (512, 256), "{texels:?}");
            assert_eq!(c.mips.len(), levels - 1);
            for (i, level) in c.mips.iter().enumerate() {
                let (w, h) = c.mip_size(i as u32);
                assert_eq!(
                    level.len(),
                    texels.level_bytes(w, h),
                    "{texels:?} level {i}"
                );
            }
            // The decode after the drop still agrees with the new level 0.
            let decoded = c.into_rgba8();
            assert_eq!(
                decoded.mips[0].len(),
                BlpTexels::Rgba8Unorm.level_bytes(512, 256)
            );
        }
        // Ineligible chains come back untouched.
        let c = drop_top_mips(full_chain(BlpTexels::Bc1, 512, 512), 2);
        assert_eq!((c.width, c.height, c.mips.len()), (512, 512, 10));
    }
}
