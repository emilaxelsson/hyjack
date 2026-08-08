use anyhow::Result;

/// Trim and validate a user-facing song or pattern name.
///
/// # Examples
///
/// ```rust
/// use dracker::state::name::normalize_and_validate_name;
///
/// assert_eq!(
///   normalize_and_validate_name("Pattern title", "  Verse 1 A  ").unwrap(),
///   "Verse 1 A"
/// );
///
/// assert_eq!(
///   normalize_and_validate_name("Pattern title", "Verse@1")
///     .unwrap_err()
///     .to_string(),
///   "Pattern title contains invalid characters"
/// );
/// ```
pub fn normalize_and_validate_name(what: &str, name: &str) -> Result<String> {
  let trimmed_name = name.trim();
  if trimmed_name.is_empty() {
    anyhow::bail!("{} cannot be empty", what);
  }
  if trimmed_name.len() > 30 {
    anyhow::bail!("{} cannot be longer than 30 characters", what);
  }
  if !trimmed_name
    .chars()
    .all(|ch| ch.is_ascii_alphanumeric() || ch == ' ' || ch == '_' || ch == '-')
  {
    anyhow::bail!("{} contains invalid characters", what);
  }
  Ok(trimmed_name.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_validate_name() {
    assert_eq!(
      normalize_and_validate_name("Name", "Valid Name 123_-").unwrap(),
      "Valid Name 123_-"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "  Valid Name 123_-  ").unwrap(),
      "Valid Name 123_-"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "")
        .unwrap_err()
        .to_string(),
      "Name cannot be empty"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "   ")
        .unwrap_err()
        .to_string(),
      "Name cannot be empty"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "Invalid@Name")
        .unwrap_err()
        .to_string(),
      "Name contains invalid characters"
    );
    assert_eq!(
      normalize_and_validate_name(
        "Name",
        "ThisNameIsWayTooLongToBeValidBecauseItExceedsThirtyCharacters"
      )
      .unwrap_err()
      .to_string(),
      "Name cannot be longer than 30 characters"
    );
  }
}
