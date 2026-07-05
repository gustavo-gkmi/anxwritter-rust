//! Error types and the validation-error contract.
//!
//! Mirrors `anxwritter/errors.py`. Validation accumulates a list of
//! [`ValidationError`] records rather than raising on the first problem; the
//! whole batch is surfaced together via [`AnxValidationError`].

use std::fmt;

use serde::{Deserialize, Serialize};

/// Central registry of validation error categories.
///
/// Serializes to the same stable snake_case strings the Python library uses,
/// so the `type` field of an error record stays wire-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    MissingRequired,
    DuplicateId,
    DuplicateName,
    MissingEntity,
    MissingTarget,
    UnknownColor,
    InvalidDate,
    InvalidTime,
    TypeConflict,
    InvalidArrow,
    SelfLoop,
    InvalidStrength,
    InvalidStrengthDefault,
    InvalidGradeDefault,
    GradeOutOfRange,
    UnknownGrade,
    InvalidOrdered,
    InvalidLegendType,
    InvalidTimezone,
    TimezoneWithoutDatetime,
    InvalidMultiplicity,
    InvalidThemeWiring,
    ConnectionConflict,
    ConfigConflict,
    PaletteTypeMismatch,
    PaletteUnknownRef,
    PaletteInvalidClass,
    InvalidValue,
    UnsupportedRepresentation,
    UnregisteredDatetimeFormat,
    InvalidSemanticType,
    UnknownSemanticType,
    InvalidMergeBehaviour,
    InvalidPasteBehaviour,
    InvalidGeoMap,
    IconMapInvalid,
    InvalidCustomIconsInclude,
    InvalidIntensityConfig,
    InvalidIntensityAttribute,
    InvalidIntensityDomain,
    InvalidIntensityRange,
    InvalidIntensityRamp,
    InvalidCategoricalConfig,
    InvalidCategoricalAttribute,
    InvalidCategoricalStyle,
    StylingConflict,
    #[serde(rename = "datetime_ac_forbids_visible")]
    DatetimeAcForbidsVisible,
    DisplayInvalid,
    DisplayNameCollision,
    DisplayOverlapConflict,
    LockedOverride,
    DeleteContract,
    IdPatternMismatch,
    AttributePatternMismatch,
    AttributeValueNotAllowed,
    RequiredAttributeMissing,
    InvalidValidatorPattern,
    ValidatorUnknownType,
    ValidatorInvalidScope,
    ValidatorInvalidShape,
    ValidatorDuplicateKey,
    ValidatorReservedAttribute,
    PatternMissingDescription,
}

/// A single validation finding.
///
/// `source` / `config_source` / `rule_source` are optional provenance tags
/// attached as errors flow through config layering and rule groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    #[serde(rename = "type")]
    pub error_type: ErrorType,
    pub location: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub config_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rule_source: Option<String>,
}

impl ValidationError {
    pub fn new(
        error_type: ErrorType,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            error_type,
            location: location.into(),
            message: message.into(),
            source: None,
            config_source: None,
            rule_source: None,
        }
    }
}

/// Raised when a chart fails validation. Carries the full batch of findings.
#[derive(Debug, Clone)]
pub struct AnxValidationError {
    pub errors: Vec<ValidationError>,
}

impl AnxValidationError {
    pub fn new(errors: Vec<ValidationError>) -> Self {
        Self { errors }
    }
}

impl fmt::Display for AnxValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chart validation failed with {} error(s)",
            self.errors.len()
        )?;
        for e in &self.errors {
            write!(
                f,
                "\n  - [{}] {}: {}",
                e.location,
                {
                    // type label as snake_case via serde
                    serde_json::to_value(e.error_type)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default()
                },
                e.message
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for AnxValidationError {}

/// Top-level crate error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Validation(#[from] AnxValidationError),
    #[error("invalid color: {0}")]
    Color(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
