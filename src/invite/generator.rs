use sha2::{Digest, Sha256};

const TERRAIN: &[&str] = &[
    "ridge", "glacier", "canyon", "summit", "meadow", "tundra", "delta", "plateau", "fjord",
    "ravine", "crater", "basin", "cape", "moraine", "col",
];

const WEATHER: &[&str] = &[
    "dawn", "dusk", "frost", "gale", "mist", "ember", "aurora", "solstice", "moonrise", "haze",
];

#[derive(Debug, Clone)]
pub struct InviteCodeData {
    #[allow(dead_code)]
    pub sequence: i32,
    pub code: String,
    pub cairn_name: String,
    pub origin_coord: (f64, f64),
}

pub fn generate_invite_code(sequence: i32, salt: &str) -> InviteCodeData {
    let seed = format!("{}:{}", salt, sequence);
    let hash = Sha256::digest(seed.as_bytes());
    let h = hash.as_slice();

    let weather = WEATHER[h[0] as usize % WEATHER.len()];
    let terrain = TERRAIN[h[1] as usize % TERRAIN.len()];
    let cairn_name = format!("{} {}", weather, terrain).to_uppercase();
    let code = format!("CAIRN-{:03}-{}", sequence, cairn_name).to_uppercase();

    let lat = (u16::from_be_bytes([h[2], h[3]]) as f64 / 65535.0) * 180.0 - 90.0;
    let lng = (u16::from_be_bytes([h[4], h[5]]) as f64 / 65535.0) * 360.0 - 180.0;

    InviteCodeData {
        sequence,
        code,
        cairn_name,
        origin_coord: (lng, lat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_invite_code() {
        let salt = "test-salt";
        let code1 = generate_invite_code(1, salt);
        let code2 = generate_invite_code(1, salt);
        let code3 = generate_invite_code(2, salt);

        assert_eq!(code1.code, code2.code);
        assert_ne!(code1.code, code3.code);
        assert!(code1.code.starts_with("CAIRN-001-"));
        assert!(code3.code.starts_with("CAIRN-002-"));

        let parts: Vec<&str> = code1.code.split('-').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "CAIRN");
        assert_eq!(parts[1], "001");
    }

    #[test]
    fn test_coordinate_bounds() {
        let salt = "test-salt";

        for i in 1..100 {
            let code = generate_invite_code(i, salt);
            let (lng, lat) = code.origin_coord;
            assert!(lat >= -90.0 && lat <= 90.0);
            assert!(lng >= -180.0 && lng <= 180.0);
        }
    }
}
