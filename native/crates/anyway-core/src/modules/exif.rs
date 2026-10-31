use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::{json, Map, Value};
use std::io::BufReader;

pub struct ExifModule;

#[async_trait::async_trait]
impl Module for ExifModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "file.exif",
            name: "File Metadata",
            description:
                "Extracts EXIF from images: device, software, timestamps and GPS coordinates",
            accepts: &[crate::models::TargetKind::File],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let path = ctx.target.raw.clone();
        ctx.progress(0, 1).await;

        let scan_path = path.clone();
        let file = tokio::task::spawn_blocking(move || -> Result<exif::Exif, CoreError> {
            let f = std::fs::File::open(&scan_path)?;
            let reader = exif::Reader::new();
            reader
                .read_from_container(&mut BufReader::new(f))
                .map_err(|e| CoreError::Parse(e.to_string()))
        })
        .await
        .map_err(|e| CoreError::Parse(e.to_string()))?;

        match file {
            Ok(exif) => {
                let mut fields = Map::new();
                for f in exif.fields() {
                    let name = f.tag.to_string();
                    if INTERESTED.contains(&name.as_str()) {
                        let val = f.display_value().with_unit(()).to_string();
                        fields.insert(name, Value::String(val.trim().to_string()));
                    }
                }

                let gps = gps_decimal(&exif);
                if let Some((lat, lon)) = gps {
                    fields.insert(
                        "gps".into(),
                        json!({
                            "lat": lat, "lon": lon,
                            "map": format!("https://www.google.com/maps?q={lat},{lon}"),
                        }),
                    );
                }

                if fields.is_empty() {
                    ctx.note("no EXIF data found").await;
                    return Ok(());
                }

                ctx.emit(Finding {
                    module_id: "file.exif".into(),
                    category: "metadata".into(),
                    label: "EXIF metadata".into(),
                    value: path.clone(),
                    url: None,
                    confidence: Confidence::High,
                    target: None,
                    detail: Some(Value::Object(fields)),
                })
                .await;
            }
            Err(e) => ctx.note(&format!("exif read failed: {e}")).await,
        }
        ctx.progress(1, 1).await;
        Ok(())
    }
}

const INTERESTED: &[&str] = &[
    "Make",
    "Model",
    "LensMake",
    "LensModel",
    "Software",
    "DateTime",
    "DateTimeOriginal",
    "DateTimeDigitized",
    "ExposureTime",
    "FNumber",
    "ISOSpeedRatings",
    "FocalLength",
    "ImageWidth",
    "ImageLength",
    "Orientation",
    "CameraOwnerName",
    "BodySerialNumber",
    "ImageUniqueID",
    "Copyright",
    "Artist",
];

fn gps_decimal(exif: &exif::Exif) -> Option<(f64, f64)> {
    let lat = dms_to_deg(exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)?)?;
    let lon = dms_to_deg(exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)?)?;
    let lat_ref = exif
        .get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY)
        .map(|f| f.display_value().with_unit(()).to_string())
        .unwrap_or_default();
    let lon_ref = exif
        .get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY)
        .map(|f| f.display_value().with_unit(()).to_string())
        .unwrap_or_default();
    let lat = if lat_ref.contains('S') { -lat } else { lat };
    let lon = if lon_ref.contains('W') { -lon } else { lon };
    Some((lat, lon))
}

fn dms_to_deg(field: &exif::Field) -> Option<f64> {
    match &field.value {
        exif::Value::Rational(rationals) if rationals.len() == 3 => {
            let d = rationals[0].to_f64();
            let m = rationals[1].to_f64();
            let s = rationals[2].to_f64();
            Some(d + m / 60.0 + s / 3600.0)
        }
        _ => None,
    }
}
