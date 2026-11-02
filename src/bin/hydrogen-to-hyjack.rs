//! Convert a Hydrogen XML song to an empty Hyjack song:
//!
//! ```text
//! hydrogen-to-hyjack <hydrogen-song.h2song> > <hyjack-song.json>
//! ```
//!
//! The song name and BPM are copied; fractional BPM is rounded to the nearest integer,
//! and values outside Hyjack's 1–255 range are rejected. Instruments are directly mapped
//! in XML order. Patterns, notes, and other Hydrogen settings are not imported. The
//! resulting stub has one empty pattern and one track.
//!
//! Each instrument must have exactly one `instrumentComponent` and an `applyVelocity`
//! element. When `applyVelocity` is `false`, layer ranges must be valid, within 0–1, and
//! adjacent in velocity order. Their internal endpoints are sorted and deduplicated to
//! form the instrument's velocity boundaries. When it is `true`, layer velocities are
//! ignored and no boundaries are added. Invalid input is reported as an error.

use std::io::{self, Write};
use std::num::NonZeroU8;
use std::{env, fs};

use anyhow::{Context, Result, ensure};
use hyjack::state::AppState;
use hyjack::state::instruments::{InstrumentMetadata, InstrumentName, Instruments};
use roxmltree::{Document, Node};

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
  node
    .children()
    .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn text(node: Node<'_, '_>, name: &str) -> Result<String> {
  let element = child(node, name).with_context(|| format!("missing <{name}>"))?;
  Ok(
    element
      .children()
      .filter_map(|child| child.text().filter(|_| child.is_text()))
      .collect(),
  )
}

fn number(node: Node<'_, '_>, name: &str) -> Result<f64> {
  let value = text(node, name)?;
  let number: f64 = value
    .trim()
    .parse()
    .with_context(|| format!("invalid <{name}> value: {value:?}"))?;
  ensure!(number.is_finite(), "invalid <{name}> value: {value:?}");
  Ok(number)
}

fn instrument(node: Node<'_, '_>) -> Result<InstrumentMetadata> {
  let name = text(node, "name").context("instrument")?;
  let apply_velocity =
    text(node, "applyVelocity").with_context(|| format!("instrument {name:?}"))?;
  let apply_velocity = match apply_velocity.trim() {
    "true" => true,
    "false" => false,
    value => anyhow::bail!("instrument {name:?}: invalid <applyVelocity> value: {value:?}"),
  };
  let mut components = node
    .children()
    .filter(|child| child.is_element() && child.tag_name().name() == "instrumentComponent");
  let component = components
    .next()
    .with_context(|| format!("instrument {name:?}: missing <instrumentComponent>"))?;
  ensure!(
    components.next().is_none(),
    "instrument {name:?}: expected exactly one <instrumentComponent>, found multiple"
  );
  if apply_velocity {
    return Ok(InstrumentMetadata {
      name: InstrumentName(name),
      velocity_boundaries: Vec::new(),
    });
  }

  let mut boundaries = Vec::new();
  let mut layers = Vec::new();
  for (layer_index, layer) in component
    .children()
    .filter(|child| child.is_element() && child.tag_name().name() == "layer")
    .enumerate()
  {
    let min = number(layer, "min").with_context(|| format!("instrument {name:?}, layer"))?;
    let max = number(layer, "max").with_context(|| format!("instrument {name:?}, layer"))?;
    ensure!(
      (0.0..=1.0).contains(&min) && (0.0..=1.0).contains(&max) && min < max,
      "instrument {name:?}: invalid layer range {min}..{max}"
    );
    layers.push((min, max, layer_index + 1));
    boundaries.extend(
      [min, max]
        .into_iter()
        .filter(|value| *value > 0.0 && *value < 1.0),
    );
  }
  layers.sort_by(|a, b| a.0.total_cmp(&b.0));
  for pair in layers.windows(2) {
    let (previous_min, previous_max, previous_index) = pair[0];
    let (next_min, next_max, next_index) = pair[1];
    ensure!(
      previous_max == next_min,
      "instrument {name:?}: layer {previous_index} ({previous_min}..{previous_max}) and layer {next_index} ({next_min}..{next_max}) are not adjacent"
    );
  }
  boundaries.sort_by(f64::total_cmp);
  boundaries.dedup();
  Ok(InstrumentMetadata {
    name: InstrumentName(name),
    velocity_boundaries: boundaries,
  })
}

fn convert(xml: &str) -> Result<AppState> {
  let document = Document::parse(xml).context("invalid Hydrogen XML")?;
  let song = document.root_element();
  ensure!(
    song.tag_name().name() == "song",
    "expected a <song> root element"
  );
  let title = text(song, "name").context("song")?;
  let bpm = number(song, "bpm").context("song")?;
  ensure!(
    (1.0..=255.0).contains(&bpm),
    "song BPM {bpm} is outside Hyjack's range 1–255"
  );
  let bpm = NonZeroU8::new(bpm.round() as u8).expect("validated BPM is nonzero");
  let instrument_list = child(song, "instrumentList").context("song: missing <instrumentList>")?;
  let instruments = instrument_list
    .children()
    .filter(|child| child.is_element() && child.tag_name().name() == "instrument")
    .map(instrument)
    .collect::<Result<Vec<_>>>()?;

  Ok(AppState::new_stub(
    title,
    bpm,
    Instruments::DirectMapped(instruments),
  ))
}

fn main() -> Result<()> {
  let mut args = env::args_os();
  let program = args.next().unwrap_or_else(|| "hydrogen-to-hyjack".into());
  let input = args.next().context(format!(
    "usage: {} <hydrogen-song.h2song> > <hyjack-song.json>",
    program.to_string_lossy()
  ))?;
  ensure!(
    args.next().is_none(),
    "usage: {} <hydrogen-song.h2song> > <hyjack-song.json>",
    program.to_string_lossy()
  );
  let xml = fs::read_to_string(&input)
    .with_context(|| format!("could not read {}", input.to_string_lossy()))?;
  let state = convert(&xml).with_context(|| format!("converting {}", input.to_string_lossy()))?;
  let stdout = io::stdout();
  let mut output = stdout.lock();
  serde_json::to_writer_pretty(&mut output, &state).context("writing Hyjack song")?;
  writeln!(output).context("writing Hyjack song")?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse_instruments(xml: &str) -> Vec<InstrumentMetadata> {
    let state = convert(xml).unwrap();
    let Instruments::DirectMapped(instruments) = state.instruments;
    instruments
  }

  #[test]
  fn preserves_metadata_and_converts_example() {
    let state = convert(include_str!("../../examples/GMRockKit_template.h2song")).unwrap();
    assert_eq!(state.song_title, "Untitled Song");
    assert_eq!(state.initial_bpm.get(), 120);
    let Instruments::DirectMapped(instruments) = state.instruments;
    assert_eq!(instruments.len(), 18);
    assert_eq!(instruments[0].name.0, "Kick");
    assert!(instruments[0].velocity_boundaries.is_empty());
    assert_eq!(instruments[17].name.0, "Bell");
  }

  #[test]
  fn ignores_layer_velocities_when_applied() {
    for layer in [
      "<layer/>",
      "<layer><min>bad</min><max>NaN</max></layer>",
      "<layer><min>-1</min><max>2</max></layer>",
      "<layer><min>0.8</min><max>0.2</max></layer>",
    ] {
      let xml = format!(
        "<song><bpm>120</bpm><name>Test</name><instrumentList>\
           <instrument><name>Snare</name><applyVelocity>true</applyVelocity>\
           <instrumentComponent>{layer}</instrumentComponent>\
           </instrument></instrumentList></song>"
      );
      assert!(parse_instruments(&xml)[0].velocity_boundaries.is_empty());

      let xml_with_false = xml.replace(
        "<applyVelocity>true</applyVelocity>",
        "<applyVelocity>false</applyVelocity>",
      );
      assert!(convert(&xml_with_false).is_err());
    }
  }

  #[test]
  fn rejects_missing_apply_velocity_with_instrument_context() {
    let xml = "<song><bpm>120</bpm><name>Test</name><instrumentList>\
      <instrument><name>Snare</name></instrument></instrumentList></song>";
    let error = match convert(xml) {
      Ok(_) => panic!("expected missing applyVelocity to fail"),
      Err(error) => format!("{error:#}"),
    };
    assert!(error.contains("instrument \"Snare\""), "{error}");
    assert!(error.contains("missing <applyVelocity>"), "{error}");
  }

  #[test]
  fn extracts_boundaries_and_preserves_instrument_order() {
    let xml = r#"<song><bpm>120.7</bpm><name>Test &amp; Song</name>
      <instrumentList>
        <instrument><id>99</id><name>A</name><applyVelocity>false</applyVelocity>
          <instrumentComponent>
            <layer><min>0.5</min><max>0.75</max></layer>
            <layer><min>0</min><max>0.25</max></layer>
            <layer><min>0.75</min><max>1</max></layer>
            <layer><min>0.25</min><max>0.5</max></layer>
          </instrumentComponent>
        </instrument>
        <instrument><id>0</id><name>B</name><applyVelocity>true</applyVelocity>
          <instrumentComponent><layer><min>0</min><max>0.5</max></layer>
            <layer><min>0.5</min><max>1</max></layer></instrumentComponent>
        </instrument>
        <instrument><name>C</name><applyVelocity>true</applyVelocity><instrumentComponent/></instrument>
      </instrumentList></song>"#;
    let state = convert(xml).unwrap();
    assert_eq!(state.song_title, "Test & Song");
    assert_eq!(state.initial_bpm.get(), 121);
    let instruments = parse_instruments(xml);
    assert_eq!(
      instruments
        .iter()
        .map(|i| i.name.0.as_str())
        .collect::<Vec<_>>(),
      ["A", "B", "C"]
    );
    assert_eq!(instruments[0].velocity_boundaries, [0.25, 0.5, 0.75]);
    assert!(instruments[1].velocity_boundaries.is_empty());
    assert!(instruments[2].velocity_boundaries.is_empty());
  }

  #[test]
  fn requires_exactly_one_component_for_each_apply_velocity_setting() {
    for apply_velocity in ["true", "false"] {
      for (components, expected_error) in [
        ("", "missing <instrumentComponent>"),
        (
          "<instrumentComponent/><instrumentComponent/>",
          "expected exactly one <instrumentComponent>, found multiple",
        ),
      ] {
        let xml = format!(
          "<song><bpm>120</bpm><name>Test</name><instrumentList>\
           <instrument><name>Snare</name><applyVelocity>{apply_velocity}</applyVelocity>\
           {components}</instrument></instrumentList></song>"
        );
        let error = match convert(&xml) {
          Ok(_) => panic!("expected invalid component count to fail"),
          Err(error) => format!("{error:#}"),
        };
        assert!(error.contains("instrument \"Snare\""), "{error}");
        assert!(error.contains(expected_error), "{error}");
      }
    }
  }

  #[test]
  fn checks_adjacency_only_when_apply_velocity_is_false() {
    for (apply_velocity, check_adjacency) in [
      ("<applyVelocity>false</applyVelocity>", true),
      ("<applyVelocity>true</applyVelocity>", false),
    ] {
      for next_min in ["0.6", "0.4"] {
        let xml = format!(
          r#"<song><bpm>120</bpm><name>Test</name><instrumentList>
          <instrument><name>Snare</name>{apply_velocity}
            <instrumentComponent>
              <layer><min>{next_min}</min><max>1</max></layer>
              <layer><min>0</min><max>0.5</max></layer>
            </instrumentComponent>
          </instrument></instrumentList></song>"#
        );
        if check_adjacency {
          let error = match convert(&xml) {
            Ok(_) => panic!("expected non-adjacent layers to fail"),
            Err(error) => error.to_string(),
          };
          assert!(error.contains("instrument \"Snare\""), "{error}");
          assert!(error.contains("layer 2 (0..0.5) and layer 1"), "{error}");
        } else {
          let instruments = parse_instruments(&xml);
          assert!(instruments[0].velocity_boundaries.is_empty());
        }
      }
    }
  }

  #[test]
  fn reports_invalid_inputs() {
    for xml in [
      "<song><bpm>0</bpm><name>x</name><instrumentList/></song>",
      "<song><bpm>255.1</bpm><name>x</name><instrumentList/></song>",
      "<song><bpm>NaN</bpm><name>x</name><instrumentList/></song>",
      "<song><bpm>120</bpm><instrumentList/></song>",
      "<song><bpm>120</bpm><name>x</name></song>",
      "<song><bpm>120</bpm><name>x</name><instrumentList><instrument><name>a</name><applyVelocity>maybe</applyVelocity></instrument></instrumentList></song>",
      "<song><bpm>120</bpm><name>x</name><instrumentList><instrument><name>a</name><applyVelocity>false</applyVelocity><instrumentComponent><layer><min>0.8</min><max>0.2</max></layer></instrumentComponent></instrument></instrumentList></song>",
      "<song>",
    ] {
      assert!(convert(xml).is_err(), "unexpected success for {xml}");
    }
  }

  #[test]
  fn accepts_namespaced_elements_and_empty_instrument_list() {
    let xml = r#"<song xmlns="urn:hydrogen"><bpm>1</bpm><name>Test</name>
        <instrumentList><instrument><name>A</name><applyVelocity>false</applyVelocity>
        <instrumentComponent><layer><min>0</min><max>0.4</max></layer>
        <layer><min>0.4</min><max>1</max></layer></instrumentComponent>
        </instrument></instrumentList></song>"#;
    assert_eq!(parse_instruments(xml)[0].velocity_boundaries, [0.4]);
    assert!(
      parse_instruments("<song><bpm>255</bpm><name>x</name><instrumentList/></song>").is_empty()
    );
  }
}
