//! [`Session::trim_memory`] checks for the integration tests (`src/lib.rs` `trim_check` is the
//! same for the decoders' own tests).

use specular_core::CancelToken;
use specular_decode::{DecodeRequest, Session};

/// Decodes `req`, trims, decodes it again: `trim_memory` releases exactly what `memory_bytes`
/// counted, leaves nothing counted, and the second decode, which rebuilds the cache, gives the
/// same pixels.
pub fn assert_trims(s: &dyn Session, req: &DecodeRequest) {
    let c = CancelToken::new();
    let a = s.decode(req, &c).unwrap();
    let held = s.memory_bytes();
    assert!(held > 0, "the decode cached something");
    assert_eq!(s.trim_memory(), held);
    assert_eq!(s.memory_bytes(), 0);
    assert_eq!(s.trim_memory(), 0, "nothing left");
    let b = s.decode(req, &c).unwrap();
    assert_eq!(
        (a.width, a.height, a.origin, a.scale, a.layout),
        (b.width, b.height, b.origin, b.scale, b.layout)
    );
    assert!(
        a.planes
            .iter()
            .zip(&b.planes)
            .all(|(p, q)| p.data == q.data)
    );
    assert_eq!(s.memory_bytes(), held, "the cache is rebuilt");
}
