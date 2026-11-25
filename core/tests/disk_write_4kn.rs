//! Write-path integration for 4K-native disks.
//!
//! Unit tests assert the strings the write path produces. This one runs the
//! real `sfdisk` on a sparse image with `--sector-size 4096` — the same
//! invocation the installer uses — and reads the table back, so a change in
//! what `sfdisk` accepts or reports is caught here, not on a customer's disk.

use camarelol_core::storage::layout::{plan_auto_layout, LayoutRequest};
use camarelol_core::storage::ops::sfdisk_script;
use camarelol_core::storage::scan::scan_devices_from_survey;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SURVEY_4KN: &str = r#"[{
    "kernel_name": "vdb", "path": "/dev/vdb", "size_bytes": 21474836480,
    "logical_sector_size": 4096, "physical_sector_size": 4096,
    "rotational": false, "removable": false, "kind": "Virtual",
    "partition_table": "None", "partitions": [],
    "mount_state": "NotMounted", "os_hints": [], "encrypted": null, "risk": "Low"
}]"#;

fn tool_present(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn scratch_image(size_bytes: u64) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("camarelol-4kn-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("disk-4kn.img");
    std::fs::File::create(&img)
        .unwrap()
        .set_len(size_bytes)
        .unwrap();
    img
}

fn write_with_sfdisk(img: &Path, script: &str) {
    let mut child = Command::new("sfdisk")
        .args(["--sector-size=4096", "--force"])
        .arg(img)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sfdisk");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("write the sfdisk script");
    child.stdin.take(); // let sfdisk see EOF, like the installer does
    let out = child.wait_with_output().expect("wait sfdisk");
    assert!(
        out.status.success(),
        "sfdisk exited with {:?}\n--- script ---\n{script}\n--- stderr ---\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn read_back(img: &Path) -> serde_json::Value {
    let out = Command::new("sfdisk")
        .args(["--json", "--sector-size=4096"])
        .arg(img)
        .output()
        .expect("run sfdisk --json");
    assert!(
        out.status.success(),
        "sfdisk --json failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("sfdisk --json is valid JSON")
}

/// A 4Kn plan writes into a real table: `sfdisk` accepts the units, the
/// table reports `sectorsize: 4096`, and every partition start is a whole
/// 1 MiB in those units.
#[test]
fn sfdisk_writes_and_reads_a_4kn_plan() {
    if !tool_present("sfdisk") {
        eprintln!("SKIP sfdisk_writes_and_reads_a_4kn_plan: sfdisk not installed");
        return;
    }

    let devices = scan_devices_from_survey(SURVEY_4KN).expect("survey parses");
    let plan = plan_auto_layout(&devices[0], &LayoutRequest::default()).expect("4Kn plans");
    let img = scratch_image(devices[0].size_bytes);
    let script = sfdisk_script(&plan, devices[0].size_bytes, 4096);

    write_with_sfdisk(&img, &script);

    let table = read_back(&img).pointer("/partitiontable").cloned().unwrap();
    assert_eq!(
        table["sectorsize"],
        serde_json::json!(4096),
        "table does not report 4096-byte sectors"
    );
    let parts = table["partitions"].as_array().expect("partitions present");
    assert!(
        !parts.is_empty(),
        "sfdisk read the table back without any partition: {table}"
    );
    for p in parts {
        let start = p["start"].as_u64().expect("start is a number");
        assert_eq!(
            start % 256,
            0,
            "partition starts must be 1 MiB aligned on a 4Kn disk: {p}"
        );
    }
    assert!(
        parts.iter().any(|p| p["start"].as_u64() == Some(256)),
        "first partition starts at 1 MiB (256 sectors of 4096): {table}"
    );

    let _ = std::fs::remove_file(&img);
    let _ = std::fs::remove_dir(img.parent().expect("parent"));
}
