use playwright_ai_test_reviewer::{Limits, extract_evidence, list_evidence};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pw-evidence-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn zip(&self, name: &str, members: &[(&str, &[u8])]) -> PathBuf {
        let path = self.0.join(name);
        let mut archive = ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in members {
            archive
                .start_file(
                    *name,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
        path
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn screenshot(resource: &str) -> Vec<u8> {
    json!({"type":"screencast-frame","pageId":"page@1","sha1":resource,"timestamp":5.0,"width":640,"height":360}).to_string().into_bytes()
}
fn snapshot() -> Vec<u8> {
    json!({"type":"frame-snapshot","snapshot":{"pageId":"page@1","frameId":"frame@1","callId":"call@1","snapshotName":"after@call@1","timestamp":6.0,"html":["HTML",{},["SCRIPT",{},"throw new Error('never execute')"]]}}).to_string().into_bytes()
}
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/browser-visual.zip")
}

#[test]
fn captured_browser_catalog_keeps_real_metadata_without_inline_content() {
    let catalog = list_evidence(&fixture(), &Limits::default()).unwrap();
    assert_eq!(catalog.total, 2);
    assert!(!catalog.truncated);
    let image = &catalog.items[0];
    assert_eq!(
        (image.kind, image.resource_status),
        ("screencast-frame", "available")
    );
    assert_eq!((image.width, image.height), (Some(640), Some(360)));
    assert!(image.page_id.as_deref().unwrap().starts_with("page@"));
    assert!(image.action_id.is_none());
    let dom = &catalog.items[1];
    assert_eq!(
        (dom.kind, dom.resource_status),
        ("frame-snapshot", "inline")
    );
    assert_eq!((dom.width, dom.height), (Some(640), Some(360)));
    assert_eq!(dom.action_id.as_deref(), Some("call@10"));
    assert_eq!(dom.snapshot_name.as_deref(), Some("after@call@10"));
    assert!(dom.timestamp_ms.is_some());
    let encoded = serde_json::to_string(&catalog).unwrap();
    assert!(!encoded.contains("Visual evidence capture"));
    assert!(!encoded.contains("html"));
    assert_eq!(
        image.id,
        list_evidence(&fixture(), &Limits::default()).unwrap().items[0].id
    );
}

#[test]
fn explicit_extraction_matches_original_jpeg_and_snapshot_record_bytes() {
    let work = Workspace::new();
    let path = fixture();
    let catalog = list_evidence(&path, &Limits::default()).unwrap();
    let mut archive = ZipArchive::new(File::open(&path).unwrap()).unwrap();
    let mut trace = String::new();
    archive
        .by_name("trace.trace")
        .unwrap()
        .read_to_string(&mut trace)
        .unwrap();
    let lines: Vec<_> = trace.lines().collect();
    let screenshot: Value = serde_json::from_str(lines[0]).unwrap();
    let mut original_image = Vec::new();
    archive
        .by_name(&format!(
            "resources/{}",
            screenshot["sha1"].as_str().unwrap()
        ))
        .unwrap()
        .read_to_end(&mut original_image)
        .unwrap();
    assert_eq!(&original_image[..2], &[0xff, 0xd8]);
    for (item, original) in catalog
        .items
        .iter()
        .zip([original_image, lines[1].as_bytes().to_vec()])
    {
        let output = work.0.join(&item.id);
        let report = extract_evidence(&path, &item.id, &output, &Limits::default()).unwrap();
        assert_eq!(fs::read(&output).unwrap(), original);
        assert_eq!(report.bytes, original.len() as u64);
        assert!(extract_evidence(&path, &item.id, &output, &Limits::default()).is_err());
        assert_eq!(fs::read(&output).unwrap(), original);
    }
    assert_eq!(fs::read_dir(&work.0).unwrap().count(), 2);
}

#[test]
fn missing_resources_and_raw_dom_remain_distinct_and_unknown_ids_do_not_write() {
    let work = Workspace::new();
    let missing = work.zip(
        "missing.zip",
        &[("trace.trace", &screenshot("missing.jpeg"))],
    );
    let item = list_evidence(&missing, &Limits::default())
        .unwrap()
        .items
        .remove(0);
    assert_eq!(item.resource_status, "missing");
    let output = work.0.join("output");
    assert!(extract_evidence(&missing, &item.id, &output, &Limits::default()).is_err());
    assert!(
        extract_evidence(
            &missing,
            "resources/missing.jpeg",
            &output,
            &Limits::default()
        )
        .is_err()
    );
    assert!(!output.exists());
    let raw = work.0.join("raw.trace");
    fs::write(&raw, screenshot("frame.jpeg")).unwrap();
    assert_eq!(
        list_evidence(&raw, &Limits::default()).unwrap().items[0].resource_status,
        "unavailable_raw_trace"
    );
    let bytes = snapshot();
    fs::write(&raw, &bytes).unwrap();
    extract_evidence(&raw, "e0-r0", &output, &Limits::default()).unwrap();
    assert_eq!(fs::read(output).unwrap(), bytes);
}

#[test]
fn catalog_caps_clip_metadata_but_ids_and_extraction_are_not_clipped() {
    let work = Workspace::new();
    let path = work.zip(
        "caps.zip",
        &[(
            "trace.trace",
            &format!(
                "{}\n{}",
                String::from_utf8(snapshot()).unwrap(),
                String::from_utf8(snapshot()).unwrap()
            )
            .into_bytes(),
        )],
    );
    let limits = Limits {
        max_evidence: 1,
        max_text: 2,
        ..Limits::default()
    };
    let catalog = list_evidence(&path, &limits).unwrap();
    assert_eq!(
        (catalog.total, catalog.items.len(), catalog.truncated),
        (2, 1, true)
    );
    assert_eq!(catalog.items[0].page_id.as_deref(), Some("pa"));
    assert!(catalog.items[0].text_truncated);
    assert_eq!(catalog.items[0].id, "e0-r0");
    extract_evidence(&path, "e0-r1", &work.0.join("second"), &limits).unwrap();
    let empty = list_evidence(
        &path,
        &Limits {
            max_evidence: 0,
            ..limits.clone()
        },
    )
    .unwrap();
    assert_eq!(
        (empty.total, empty.items.len(), empty.truncated),
        (2, 0, true)
    );
    assert!(
        list_evidence(
            &path,
            &Limits {
                max_events: 1,
                ..limits.clone()
            }
        )
        .is_err()
    );
    assert!(
        list_evidence(
            &path,
            &Limits {
                max_bytes: 2,
                ..limits
            }
        )
        .is_err()
    );
}

#[test]
fn unsafe_members_resource_references_duplicate_members_and_malformed_records_fail_closed() {
    let work = Workspace::new();
    for (index, name) in [
        "../trace.trace",
        "/trace.trace",
        "dir/../../trace.trace",
        "dir\\trace.trace",
    ]
    .iter()
    .enumerate()
    {
        let path = work.zip(&format!("unsafe{index}.zip"), &[(name, &snapshot())]);
        assert!(list_evidence(&path, &Limits::default()).is_err(), "{name}");
    }
    for (index, resource) in [
        "../private",
        "/private",
        "a/b",
        "a\\b",
        "https://host/image",
        "..",
        "",
    ]
    .iter()
    .enumerate()
    {
        let path = work.zip(
            &format!("reference{index}.zip"),
            &[("trace.trace", &screenshot(resource))],
        );
        assert!(
            list_evidence(&path, &Limits::default()).is_err(),
            "{resource}"
        );
    }
    for (index, record) in [
        b"{invalid}".as_slice(),
        br#"{"type":"frame-snapshot"}"#,
        br#"{"type":"screencast-frame"}"#,
    ]
    .iter()
    .enumerate()
    {
        let path = work.zip(&format!("bad{index}.zip"), &[("trace.trace", record)]);
        assert!(list_evidence(&path, &Limits::default()).is_err());
    }
    let duplicate = work.zip(
        "duplicate.zip",
        &[("a.trace", &snapshot()), ("b.trace", &snapshot())],
    );
    let bytes = fs::read(&duplicate).unwrap();
    let replaced: Vec<_> = bytes
        .windows(7)
        .enumerate()
        .filter_map(|(i, b)| (b == b"b.trace").then_some(i))
        .collect();
    let mut bytes = bytes;
    for index in replaced {
        bytes[index] = b'a';
    }
    fs::write(&duplicate, bytes).unwrap();
    assert!(
        list_evidence(&duplicate, &Limits::default())
            .unwrap_err()
            .to_string()
            .contains("Duplicate")
    );
}

#[test]
fn compressed_trace_and_screenshot_budgets_apply_before_output_creation() {
    let work = Workspace::new();
    let huge = vec![b' '; 64 * 1024];
    let path = work.zip("bomb.zip", &[("trace.trace", &huge)]);
    assert!(fs::metadata(&path).unwrap().len() < 2048);
    assert!(
        list_evidence(
            &path,
            &Limits {
                max_bytes: 2048,
                ..Limits::default()
            }
        )
        .is_err()
    );
    let path = work.zip(
        "image-bomb.zip",
        &[
            ("trace.trace", &screenshot("image.jpeg")),
            ("resources/image.jpeg", &huge),
        ],
    );
    assert!(
        list_evidence(
            &path,
            &Limits {
                max_bytes: 2048,
                ..Limits::default()
            }
        )
        .is_err()
    );
    let path = work.zip(
        "combined.zip",
        &[
            ("trace.trace", &screenshot("image.jpeg")),
            ("resources/image.jpeg", &vec![0; 1900]),
        ],
    );
    let output = work.0.join("output");
    assert!(
        extract_evidence(
            &path,
            "e0-r0",
            &output,
            &Limits {
                max_bytes: 2000,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(!output.exists());
}

#[cfg(unix)]
#[test]
fn output_and_archive_symlinks_cannot_escape_or_overwrite() {
    use std::os::unix::fs::symlink;
    let work = Workspace::new();
    let outside = Workspace::new();
    symlink(&outside.0, work.0.join("linked-parent")).unwrap();
    let path = fixture();
    assert!(
        extract_evidence(
            &path,
            "e0-r0",
            &work.0.join("linked-parent/escape"),
            &Limits::default()
        )
        .is_err()
    );
    assert!(!outside.0.join("escape").exists());
    fs::write(outside.0.join("existing"), b"preserve").unwrap();
    symlink(outside.0.join("existing"), work.0.join("linked-file")).unwrap();
    assert!(
        extract_evidence(
            &path,
            "e0-r0",
            &work.0.join("linked-file"),
            &Limits::default()
        )
        .is_err()
    );
    assert_eq!(fs::read(outside.0.join("existing")).unwrap(), b"preserve");
    let archive_path = work.0.join("symlink.zip");
    let mut archive = ZipWriter::new(File::create(&archive_path).unwrap());
    archive
        .add_symlink("trace.trace", "outside", SimpleFileOptions::default())
        .unwrap();
    archive.finish().unwrap();
    assert!(list_evidence(&archive_path, &Limits::default()).is_err());
}

#[test]
fn full_scan_covers_later_entries_after_catalog_is_full_and_absence_is_not_an_error() {
    let work = Workspace::new();
    let limits = Limits {
        max_evidence: 1,
        ..Limits::default()
    };
    let path = work.zip(
        "entries.zip",
        &[("z.trace", &snapshot()), ("a.trace", &snapshot())],
    );
    let catalog = list_evidence(&path, &limits).unwrap();
    assert_eq!(
        (catalog.total, catalog.items.len(), catalog.truncated),
        (2, 1, true)
    );
    assert_eq!(catalog.items[0].id, "e0-r0");
    extract_evidence(&path, "e1-r0", &work.0.join("later"), &limits).unwrap();
    let broken = work.zip(
        "later-malformed.zip",
        &[("a.trace", &snapshot()), ("z.trace", b"{malformed}")],
    );
    assert!(list_evidence(&broken, &limits).is_err());
    let empty = work.0.join("empty.trace");
    fs::write(&empty, b"[]").unwrap();
    let catalog = list_evidence(&empty, &limits).unwrap();
    assert_eq!(
        (catalog.total, catalog.items.len(), catalog.truncated),
        (0, 0, false)
    );
}
