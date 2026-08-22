use std::io::Read;
use flate2::read::ZlibDecoder;
use std::sync::OnceLock;

const SEED_KEY: &[u8] = b"SignalHunter@ISPFocus*CoreAsset#2026";

/// Desofusca e descomprime o payload diretamente na memória RAM
fn unscramble_asset(scrambled: &[u8]) -> Vec<u8> {
    let mut compressed = Vec::with_capacity(scrambled.len());
    let key_len = SEED_KEY.len();
    
    for (i, &b) in scrambled.iter().enumerate() {
        let k = SEED_KEY[i % key_len];
        let orig_byte = ((b ^ 0x5A).wrapping_sub((i & 0xFF) as u8)) ^ k;
        compressed.push(orig_byte);
    }
    
    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    if let Err(e) = decoder.read_to_end(&mut decompressed) {
        log::error!("Falha ao desofuscar asset protegido: {}", e);
    }
    decompressed
}

/// Carrega a logo corporativa protegida com cache em memória RAM
pub fn get_embedded_logo() -> &'static [u8] {
    static LOGO_CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    LOGO_CACHE.get_or_init(|| {
        static RAW_SCRAMBLED: &[u8] = include_bytes!("embedded_assets/logo.dat");
        unscramble_asset(RAW_SCRAMBLED)
    })
}

/// Carrega o background hero protegido com cache em memória RAM
pub fn get_embedded_hero_bg() -> &'static [u8] {
    static HERO_CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    HERO_CACHE.get_or_init(|| {
        static RAW_SCRAMBLED: &[u8] = include_bytes!("embedded_assets/hero_bg.dat");
        unscramble_asset(RAW_SCRAMBLED)
    })
}

/// Carrega a logo para relatórios PDF protegida com cache em memória RAM
pub fn get_embedded_pdf_logo() -> &'static [u8] {
    static PDF_LOGO_CACHE: OnceLock<Vec<u8>> = OnceLock::new();
    PDF_LOGO_CACHE.get_or_init(|| {
        static RAW_SCRAMBLED: &[u8] = include_bytes!("embedded_assets/pdf_logo.dat");
        unscramble_asset(RAW_SCRAMBLED)
    })
}
