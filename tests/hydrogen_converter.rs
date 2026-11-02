use std::process::Command;

use hyjack::state::AppState;
use hyjack::state::instruments::Instruments;

#[test]
fn converter_writes_a_loadable_stub() {
  let output = Command::new(env!("CARGO_BIN_EXE_hydrogen-to-hyjack"))
    .arg(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/examples/GMRockKit_template.h2song"
    ))
    .output()
    .unwrap();
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let state: AppState = serde_json::from_slice(&output.stdout).unwrap();
  assert_eq!(state.song_title, "Untitled Song");
  assert_eq!(state.get_current_pattern().title, "Pattern 1");
  assert!(state.get_current_pattern().tracks.is_empty());
  let Instruments::DirectMapped(instruments) = state.instruments;
  assert_eq!(instruments.len(), 18);
  assert!(instruments[0].velocity_boundaries.is_empty());
}

#[test]
fn converter_reports_cli_errors_without_output() {
  let output = Command::new(env!("CARGO_BIN_EXE_hydrogen-to-hyjack"))
    .output()
    .unwrap();
  assert!(!output.status.success());
  assert!(output.stdout.is_empty());
  assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}
