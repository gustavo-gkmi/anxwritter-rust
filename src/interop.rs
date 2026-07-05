//! i2 interoperability constants, mirroring `anxwritter/_i2_interop.py` and the
//! GUID generation in `anxwritter/semantic.py`.
//!
//! These nine GUIDs (six abstract roots + three geo-map property anchors) are
//! the only i2-derived content shipped — functional format tokens, not content.

use uuid::Uuid;

// ── Abstract roots ──────────────────────────────────────────────────────────
pub const ROOT_ENTITY: &str = "guid3669EC21-8E41-438A-AA1A-26B477C15BE0";
pub const ROOT_LINK: &str = "guidC9E54967-BBBF-494B-8348-B9D524F500FD";
pub const ROOT_ABSTRACT_TEXT: &str = "guid9A224CCF-28F7-4c55-9F14-9E820A0B1631";
pub const ROOT_ABSTRACT_NUM: &str = "guid6D676796-915D-487f-B384-73503C988ABE";
pub const ROOT_ABSTRACT_DT: &str = "guid6684F871-B607-4ffb-80E8-480535CB44FC";
pub const ROOT_ABSTRACT_FLAG: &str = "guid74F2A516-2F49-4282-989F-F4A468656FF0";

// ── Geo-map property GUIDs ──────────────────────────────────────────────────
pub const LATITUDE_GUID: &str = "guid5304A03B-FE47-4406-91E7-0D49EC8409A6";
pub const LONGITUDE_GUID: &str = "guid14BCA0EC-D67A-4A67-BC36-CFF650FD77A9";
pub const GRID_REFERENCE_GUID: &str = "guid7E0F705E-3D39-4E6E-B6C1-5E72B8C573DA";

// ── LCX schema metadata ─────────────────────────────────────────────────────
pub const LCX_NS: &str = "http://www.i2group.com/Schemas/2001-12-07/LCXSchema";
pub const LCX_VERSION_MAJOR: &str = "1";
pub const LCX_VERSION_MINOR: &str = "18";
pub const LCX_VERSION_RELEASE: &str = "27";
pub const LCX_VERSION_BUILD: &str = "60";
pub const LCX_LOCALE_HEX: &str = "0809";

/// Namespace for deterministic semantic-type GUIDs (`semantic._ANXWRITTER_NS`).
const ANXWRITTER_NS: Uuid = Uuid::from_bytes([
    0xd1, 0xe2, 0xf3, 0xa4, 0xb5, 0xc6, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90,
]);

/// Deterministic GUID from a key. Same key always yields the same result.
///
/// Convention: `entity:Suspect`, `link:Surveilled`, `property:CPF Number`.
pub fn generate_guid(key: &str) -> String {
    let raw = Uuid::new_v5(&ANXWRITTER_NS, key.as_bytes());
    format!("guid{}", raw.to_string().to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_is_deterministic_and_prefixed() {
        let a = generate_guid("entity:Suspect");
        let b = generate_guid("entity:Suspect");
        assert_eq!(a, b);
        assert!(a.starts_with("guid"));
        assert_eq!(a.len(), 4 + 36);
    }

    #[test]
    fn distinct_keys_differ() {
        assert_ne!(generate_guid("entity:A"), generate_guid("entity:B"));
    }
}
