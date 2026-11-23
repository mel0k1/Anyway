use base64::Engine as _;
use image::{DynamicImage, GenericImageView, ImageBuffer, Luma};
use serde_json::{json, Value};
use std::time::Duration;

const HASH_SIZE: u32 = 8;
const DCT_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct ImageHashes {
    pub ahash: u64,
    pub dhash: u64,
    pub phash: u64,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

impl ImageHashes {
    pub fn to_json(&self) -> Value {
        json!({
            "ahash": format!("{:016x}", self.ahash),
            "dhash": format!("{:016x}", self.dhash),
            "phash": format!("{:016x}", self.phash),
            "width": self.width,
            "height": self.height,
            "format": self.format,
        })
    }
}

pub fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("decode failed: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("empty image".into());
    }
    Ok(img)
}

pub fn hashes_from_bytes(bytes: &[u8]) -> Result<ImageHashes, String> {
    let img = decode(bytes)?;
    let format = image::guess_format(bytes)
        .map(|f| format!("{f:?}").to_ascii_lowercase())
        .unwrap_or_else(|_| "unknown".into());
    let (width, height) = img.dimensions();
    let gray = grayscale_32(&img);
    let gray8 = img
        .resize_exact(HASH_SIZE, HASH_SIZE, image::imageops::FilterType::Triangle)
        .to_luma8();
    Ok(ImageHashes {
        ahash: ahash64(&gray8),
        dhash: dhash64(&img),
        phash: phash64(&gray),
        width,
        height,
        format,
    })
}

pub fn hashes_from_b64(data_b64: &str) -> Result<ImageHashes, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64.trim())
        .map_err(|e| format!("bad base64: {e}"))?;
    hashes_from_bytes(&bytes)
}

fn grayscale_32(img: &DynamicImage) -> ImageBuffer<Luma<f32>, Vec<f32>> {
    let small = img
        .resize_exact(
            DCT_SIZE as u32,
            DCT_SIZE as u32,
            image::imageops::FilterType::Triangle,
        )
        .to_luma8();
    let mut out = ImageBuffer::new(DCT_SIZE as u32, DCT_SIZE as u32);
    for y in 0..DCT_SIZE {
        for x in 0..DCT_SIZE {
            out.put_pixel(
                x as u32,
                y as u32,
                Luma([small.get_pixel(x as u32, y as u32)[0] as f32]),
            );
        }
    }
    out
}

pub fn phash64(gray: &ImageBuffer<Luma<f32>, Vec<f32>>) -> u64 {
    let n = DCT_SIZE;
    let rows: Vec<Vec<f64>> = (0..n)
        .map(|y| {
            let row: Vec<f64> = (0..n)
                .map(|x| gray.get_pixel(x as u32, y as u32)[0] as f64)
                .collect();
            dct1d(&row)
        })
        .collect();

    let freq: Vec<Vec<f64>> = (0..HASH_SIZE as usize)
        .map(|u| {
            (0..HASH_SIZE as usize)
                .map(|v| {
                    let col: Vec<f64> = rows.iter().map(|r| r[v]).collect();
                    dct1d(&col)[u]
                })
                .collect()
        })
        .collect();
    let mut mids: Vec<f64> = Vec::with_capacity(63);
    let mut bits: Vec<bool> = Vec::with_capacity(63);
    for (u, row) in freq.iter().enumerate() {
        for (v, val) in row.iter().enumerate() {
            if u == 0 && v == 0 {
                continue;
            }
            mids.push(*val);
        }
    }
    mids.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = mids[mids.len() / 2];
    for (u, row) in freq.iter().enumerate() {
        for (v, val) in row.iter().enumerate() {
            if u == 0 && v == 0 {
                continue;
            }
            bits.push(*val > median);
        }
    }
    pack_bits(&bits)
}

pub fn dhash64(img: &DynamicImage) -> u64 {
    let small = img
        .resize_exact(
            HASH_SIZE + 1,
            HASH_SIZE,
            image::imageops::FilterType::Triangle,
        )
        .to_luma8();
    let mut bits = Vec::with_capacity(64);
    for y in 0..HASH_SIZE {
        for x in 0..HASH_SIZE {
            let left = small.get_pixel(x, y)[0] as i16;
            let right = small.get_pixel(x + 1, y)[0] as i16;
            bits.push(left > right);
        }
    }
    pack_bits(&bits)
}

pub fn ahash64(gray8: &image::GrayImage) -> u64 {
    let mut sum = 0.0f64;
    for p in gray8.pixels() {
        sum += p[0] as f64;
    }
    let mean = sum / (HASH_SIZE * HASH_SIZE) as f64;
    let mut bits = Vec::with_capacity(64);
    for p in gray8.pixels() {
        bits.push(p[0] as f64 > mean);
    }
    pack_bits(&bits)
}

fn pack_bits(bits: &[bool]) -> u64 {
    let mut out = 0u64;
    for (i, b) in bits.iter().take(64).enumerate() {
        if *b {
            out |= 1 << i;
        }
    }
    out
}

fn dct1d(input: &[f64]) -> Vec<f64> {
    let n = input.len();
    let mut out = vec![0.0f64; n];
    for (u, o) in out.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (x, v) in input.iter().enumerate() {
            let angle = std::f64::consts::PI / (2.0 * n as f64) * ((2 * x + 1) * u) as f64;
            sum += v * angle.cos();
        }
        *o = sum
            * if u == 0 {
                (1.0 / n as f64).sqrt()
            } else {
                (2.0 / n as f64).sqrt()
            };
    }
    out
}

fn hex_to_u64(hex: &str) -> Option<u64> {
    u64::from_str_radix(hex.trim().trim_start_matches("0x"), 16).ok()
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[derive(Debug, Clone)]
pub struct HashComparison {
    pub phash: u32,
    pub dhash: u32,
    pub ahash: u32,
    pub similar: bool,
}

impl HashComparison {
    pub fn to_json(&self) -> Value {
        json!({
            "phashDistance": self.phash,
            "dhashDistance": self.dhash,
            "ahashDistance": self.ahash,
            "similar": self.similar,
        })
    }
}

pub fn compare_hashes(a: &Value, b: &Value) -> Result<HashComparison, String> {
    let get = |v: &Value, key: &str| -> Result<u64, String> {
        v[key]
            .as_str()
            .and_then(hex_to_u64)
            .ok_or(format!("missing or bad '{key}' hash"))
    };
    let (ap, bp) = (get(a, "phash")?, get(b, "phash")?);
    let (ad, bd) = (get(a, "dhash")?, get(b, "dhash")?);
    let (aa, ba) = (get(a, "ahash")?, get(b, "ahash")?);
    let ph = hamming(ap, bp);
    let dh = hamming(ad, bd);
    let ah = hamming(aa, ba);
    let similar = ph <= 10 && (dh <= 10 || ah <= 12);
    Ok(HashComparison {
        phash: ph,
        dhash: dh,
        ahash: ah,
        similar,
    })
}

pub fn index_add(index: &Value, entry: Value) -> Result<Value, String> {
    if entry.get("phash").and_then(Value::as_str).is_none() {
        return Err("entry needs a 'phash'".into());
    }
    let mut arr = index.as_array().cloned().unwrap_or_default();
    if arr.len() >= 20_000 {
        return Err("index is full (20000 entries max)".into());
    }
    arr.push(entry);
    Ok(Value::Array(arr))
}

pub fn index_search(
    index: &Value,
    probe: &Value,
    threshold: u32,
    limit: usize,
) -> Result<Value, String> {
    let entries = index
        .as_array()
        .ok_or("index must be a JSON array of entries")?;
    let mut matches: Vec<Value> = Vec::new();
    for (idx, e) in entries.iter().enumerate() {
        if let Ok(cmp) = compare_hashes(probe, e) {
            if cmp.phash <= threshold && cmp.similar {
                matches.push(json!({
                    "index": idx,
                    "entry": e,
                    "phashDistance": cmp.phash,
                    "dhashDistance": cmp.dhash,
                    "ahashDistance": cmp.ahash,
                }));
            }
        }
    }
    matches.sort_by_key(|m| m["phashDistance"].as_u64().unwrap_or(u64::MAX));
    matches.truncate(limit.max(1));
    Ok(json!({"ok": true, "matches": matches, "total": matches.len()}))
}

pub fn reverse_links(image_url: &str) -> Value {
    let url = image_url.trim();
    if url.is_empty() || !url.starts_with("http") {
        return json!({"ok": false, "error": "need an http(s) image url"});
    }
    let encoded = urlencoding_lite(url);
    json!({
        "ok": true,
        "engines": [
            {"name": "Google Lens", "url": format!("https://lens.google.com/uploadbyurl?url={encoded}")},
            {"name": "Yandex Images", "url": format!("https://yandex.com/images/search?rpt=imageview&url={encoded}")},
            {"name": "Bing Visual Search", "url": format!("https://www.bing.com/images/search?view=detailv2&iss=sbi&q=imgurl:{encoded}")},
            {"name": "TinEye", "url": format!("https://tineye.com/search?url={encoded}")},
        ],
        "note": "for local files upload the image manually or hash-match it against the local index"
    })
}

fn urlencoding_lite(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub async fn ocr_bytes(bytes: &[u8], langs: &str, psm: &str) -> Result<Value, String> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("image too large for OCR (>64MB)".into());
    }
    let ext = image::guess_format(bytes)
        .map(|f| format!("{f:?}").to_ascii_lowercase())
        .unwrap_or_else(|_| "png".into());
    let tmp = std::env::temp_dir().join(format!(
        "anyway_ocr_{}.{ext}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|e| format!("temp write failed: {e}"))?;
    let langs = if langs.trim().is_empty() {
        "eng"
    } else {
        langs.trim()
    };
    let psm = if psm.trim().is_empty() {
        "3"
    } else {
        psm.trim()
    };
    let out = tokio::time::timeout(
        Duration::from_secs(120),
        tokio::process::Command::new("tesseract")
            .arg(&tmp)
            .arg("stdout")
            .arg("-l")
            .arg(langs)
            .arg("--psm")
            .arg(psm)
            .output(),
    )
    .await;
    let _ = tokio::fs::remove_file(&tmp).await;
    match out {
        Err(_) => Err("OCR timed out (120s)".into()),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            Err("tesseract binary not found on PATH".into())
        }
        Ok(Err(e)) => Err(format!("tesseract spawn failed: {e}")),
        Ok(Ok(o)) => {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !o.status.success() {
                let err = String::from_utf8_lossy(&o.stderr);
                return Err(format!("tesseract failed: {}", err.trim()));
            }
            Ok(json!({
                "ok": true,
                "text": text,
                "chars": text.chars().count(),
                "langs": langs,
                "psm": psm,
            }))
        }
    }
}

pub async fn ocr_available() -> Value {
    let out = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new("tesseract")
            .arg("--list-langs")
            .output(),
    )
    .await;
    match out {
        Ok(Ok(o)) if o.status.success() => {
            let mut langs: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .skip(1)
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            langs.sort();
            json!({"ok": true, "available": true, "langs": langs})
        }
        _ => json!({"ok": true, "available": false}),
    }
}

pub async fn intel_bytes(bytes: &[u8], ocr: bool, langs: &str) -> Result<Value, String> {
    let hashes = hashes_from_bytes(bytes)?;
    let mut v = hashes.to_json();
    v["ok"] = json!(true);
    if ocr {
        match ocr_bytes(bytes, langs, "3").await {
            Ok(r) => v["ocr"] = r,
            Err(e) => v["ocr"] = json!({"ok": false, "error": e}),
        }
    }
    Ok(v)
}

pub async fn intel_bytes_base64(data_b64: &str, ocr: bool, langs: &str) -> Result<Value, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64.trim())
        .map_err(|e| format!("bad base64: {e}"))?;
    intel_bytes(&bytes, ocr, langs).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn png_bytes(img: &RgbImage) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn flat(color: [u8; 3]) -> Vec<u8> {
        let mut img = RgbImage::new(64, 64);
        for p in img.pixels_mut() {
            *p = Rgb(color);
        }
        png_bytes(&img)
    }

    fn stripes() -> Vec<u8> {
        let mut img = RgbImage::new(64, 64);
        for (x, _, p) in img.enumerate_pixels_mut() {
            let v = if (x / 4) % 2 == 0 { 255 } else { 0 };
            *p = Rgb([v, v, v]);
        }
        png_bytes(&img)
    }

    fn halves() -> Vec<u8> {
        let mut img = RgbImage::new(64, 64);
        for (x, _, p) in img.enumerate_pixels_mut() {
            let v = if x < 32 { 255 } else { 0 };
            *p = Rgb([v, v, v]);
        }
        png_bytes(&img)
    }

    #[test]
    fn hashes_self_consistent() {
        let bytes = stripes();
        let h = hashes_from_bytes(&bytes).unwrap();
        assert_eq!(h.width, 64);
        assert!(h.format.contains("png"));
        let h2 = hashes_from_bytes(&bytes).unwrap();
        assert_eq!(h.phash, h2.phash);
        assert_eq!(hamming(h.phash, h2.phash), 0);
    }

    #[test]
    fn different_images_differ() {
        let a = hashes_from_bytes(&flat([255, 0, 0])).unwrap();
        let c = hashes_from_bytes(&halves()).unwrap();
        assert_ne!(a.phash, c.phash);
        assert_ne!(a.dhash, c.dhash);
        assert_ne!(a.ahash, c.ahash);

        let b = hashes_from_bytes(&flat([0, 0, 255])).unwrap();
        assert_eq!(a.ahash, b.ahash);
        assert_eq!(a.dhash, b.dhash);
    }

    #[test]
    fn jpeg_reencode_stays_close() {
        let src = stripes();
        let img = decode(&src).unwrap();
        let mut jpg = std::io::Cursor::new(Vec::new());
        img.write_to(&mut jpg, image::ImageFormat::Jpeg).unwrap();
        let h1 = hashes_from_bytes(&src).unwrap();
        let h2 = hashes_from_bytes(&jpg.into_inner()).unwrap();
        assert!(hamming(h1.phash, h2.phash) <= 10);
        assert!(hamming(h1.dhash, h2.dhash) <= 6);
    }

    #[test]
    fn base64_roundtrip() {
        let h = hashes_from_bytes(&stripes()).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(stripes());
        let h2 = hashes_from_b64(&b64).unwrap();
        assert_eq!(h.phash, h2.phash);
        assert!(hashes_from_b64("!!!").is_err());
    }

    #[test]
    fn compare_and_search() {
        let a = hashes_from_bytes(&stripes()).unwrap();
        let b = hashes_from_bytes(&stripes()).unwrap();
        let c = hashes_from_bytes(&flat([10, 10, 10])).unwrap();
        let same = compare_hashes(&a.to_json(), &b.to_json()).unwrap();
        assert!(same.similar);
        assert_eq!(same.phash, 0);
        let diff = compare_hashes(&a.to_json(), &c.to_json()).unwrap();
        assert!(!diff.similar);
        let index = json!([
            {"label": "screenshot-1", "phash": a.to_json()["phash"], "dhash": a.to_json()["dhash"], "ahash": a.to_json()["ahash"]},
            {"label": "flat", "phash": c.to_json()["phash"], "dhash": c.to_json()["dhash"], "ahash": c.to_json()["ahash"]}
        ]);
        let res = index_search(&index, &a.to_json(), 10, 10).unwrap();
        let matches = res["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["entry"]["label"], "screenshot-1");
        assert!(index_add(&json!([]), json!({"phash": "ab"})).is_ok());
        assert!(index_add(&json!([]), json!({"nope": 1})).is_err());
    }

    #[test]
    fn reverse_links_shape() {
        let v = reverse_links("https://example.com/pic.jpg");
        assert_eq!(v["engines"].as_array().unwrap().len(), 4);
        let lens = v["engines"][0]["url"].as_str().unwrap();
        assert!(lens.contains("https%3A%2F%2Fexample.com%2Fpic.jpg"));
        assert_eq!(reverse_links("")["ok"], false);
        assert_eq!(reverse_links("ftp://x")["ok"], false);
    }

    #[tokio::test]
    async fn ocr_graceful_without_or_with_tesseract() {
        if let Ok(v) = ocr_bytes(&flat([255, 255, 255]), "eng", "7").await {
            assert_eq!(v["ok"], true);
        }
        let _ = ocr_available().await;
    }

    #[tokio::test]
    async fn intel_bundle() {
        let v = intel_bytes(&stripes(), false, "").await.unwrap();
        assert_eq!(v["ok"], true);
        assert!(v["phash"].is_string());
    }
}
