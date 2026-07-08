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

impl ErrorType {
    /// The stable snake_case wire string for this code — the same value serde
    /// serializes (and Python emits). Lets you log or compare a code without a
    /// serde round-trip.
    pub fn as_str(&self) -> &'static str {
        use ErrorType::*;
        match self {
            MissingRequired => "missing_required",
            DuplicateId => "duplicate_id",
            DuplicateName => "duplicate_name",
            MissingEntity => "missing_entity",
            MissingTarget => "missing_target",
            UnknownColor => "unknown_color",
            InvalidDate => "invalid_date",
            InvalidTime => "invalid_time",
            TypeConflict => "type_conflict",
            InvalidArrow => "invalid_arrow",
            SelfLoop => "self_loop",
            InvalidStrength => "invalid_strength",
            InvalidStrengthDefault => "invalid_strength_default",
            InvalidGradeDefault => "invalid_grade_default",
            GradeOutOfRange => "grade_out_of_range",
            UnknownGrade => "unknown_grade",
            InvalidOrdered => "invalid_ordered",
            InvalidLegendType => "invalid_legend_type",
            InvalidTimezone => "invalid_timezone",
            TimezoneWithoutDatetime => "timezone_without_datetime",
            InvalidMultiplicity => "invalid_multiplicity",
            InvalidThemeWiring => "invalid_theme_wiring",
            ConnectionConflict => "connection_conflict",
            ConfigConflict => "config_conflict",
            PaletteTypeMismatch => "palette_type_mismatch",
            PaletteUnknownRef => "palette_unknown_ref",
            PaletteInvalidClass => "palette_invalid_class",
            InvalidValue => "invalid_value",
            UnsupportedRepresentation => "unsupported_representation",
            UnregisteredDatetimeFormat => "unregistered_datetime_format",
            InvalidSemanticType => "invalid_semantic_type",
            UnknownSemanticType => "unknown_semantic_type",
            InvalidMergeBehaviour => "invalid_merge_behaviour",
            InvalidPasteBehaviour => "invalid_paste_behaviour",
            InvalidGeoMap => "invalid_geo_map",
            IconMapInvalid => "icon_map_invalid",
            InvalidCustomIconsInclude => "invalid_custom_icons_include",
            InvalidIntensityConfig => "invalid_intensity_config",
            InvalidIntensityAttribute => "invalid_intensity_attribute",
            InvalidIntensityDomain => "invalid_intensity_domain",
            InvalidIntensityRange => "invalid_intensity_range",
            InvalidIntensityRamp => "invalid_intensity_ramp",
            InvalidCategoricalConfig => "invalid_categorical_config",
            InvalidCategoricalAttribute => "invalid_categorical_attribute",
            InvalidCategoricalStyle => "invalid_categorical_style",
            StylingConflict => "styling_conflict",
            DatetimeAcForbidsVisible => "datetime_ac_forbids_visible",
            DisplayInvalid => "display_invalid",
            DisplayNameCollision => "display_name_collision",
            DisplayOverlapConflict => "display_overlap_conflict",
            LockedOverride => "locked_override",
            DeleteContract => "delete_contract",
            IdPatternMismatch => "id_pattern_mismatch",
            AttributePatternMismatch => "attribute_pattern_mismatch",
            AttributeValueNotAllowed => "attribute_value_not_allowed",
            RequiredAttributeMissing => "required_attribute_missing",
            InvalidValidatorPattern => "invalid_validator_pattern",
            ValidatorUnknownType => "validator_unknown_type",
            ValidatorInvalidScope => "validator_invalid_scope",
            ValidatorInvalidShape => "validator_invalid_shape",
            ValidatorDuplicateKey => "validator_duplicate_key",
            ValidatorReservedAttribute => "validator_reserved_attribute",
            PatternMissingDescription => "pattern_missing_description",
        }
    }
}

impl fmt::Display for ErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
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
                e.error_type.as_str(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant's `as_str()` must equal its serde serialization (the wire
    /// string) — guards against a typo drifting the two apart.
    const ALL: &[ErrorType] = &[
        ErrorType::MissingRequired,
        ErrorType::DuplicateId,
        ErrorType::DuplicateName,
        ErrorType::MissingEntity,
        ErrorType::MissingTarget,
        ErrorType::UnknownColor,
        ErrorType::InvalidDate,
        ErrorType::InvalidTime,
        ErrorType::TypeConflict,
        ErrorType::InvalidArrow,
        ErrorType::SelfLoop,
        ErrorType::InvalidStrength,
        ErrorType::InvalidStrengthDefault,
        ErrorType::InvalidGradeDefault,
        ErrorType::GradeOutOfRange,
        ErrorType::UnknownGrade,
        ErrorType::InvalidOrdered,
        ErrorType::InvalidLegendType,
        ErrorType::InvalidTimezone,
        ErrorType::TimezoneWithoutDatetime,
        ErrorType::InvalidMultiplicity,
        ErrorType::InvalidThemeWiring,
        ErrorType::ConnectionConflict,
        ErrorType::ConfigConflict,
        ErrorType::PaletteTypeMismatch,
        ErrorType::PaletteUnknownRef,
        ErrorType::PaletteInvalidClass,
        ErrorType::InvalidValue,
        ErrorType::UnsupportedRepresentation,
        ErrorType::UnregisteredDatetimeFormat,
        ErrorType::InvalidSemanticType,
        ErrorType::UnknownSemanticType,
        ErrorType::InvalidMergeBehaviour,
        ErrorType::InvalidPasteBehaviour,
        ErrorType::InvalidGeoMap,
        ErrorType::IconMapInvalid,
        ErrorType::InvalidCustomIconsInclude,
        ErrorType::InvalidIntensityConfig,
        ErrorType::InvalidIntensityAttribute,
        ErrorType::InvalidIntensityDomain,
        ErrorType::InvalidIntensityRange,
        ErrorType::InvalidIntensityRamp,
        ErrorType::InvalidCategoricalConfig,
        ErrorType::InvalidCategoricalAttribute,
        ErrorType::InvalidCategoricalStyle,
        ErrorType::StylingConflict,
        ErrorType::DatetimeAcForbidsVisible,
        ErrorType::DisplayInvalid,
        ErrorType::DisplayNameCollision,
        ErrorType::DisplayOverlapConflict,
        ErrorType::LockedOverride,
        ErrorType::DeleteContract,
        ErrorType::IdPatternMismatch,
        ErrorType::AttributePatternMismatch,
        ErrorType::AttributeValueNotAllowed,
        ErrorType::RequiredAttributeMissing,
        ErrorType::InvalidValidatorPattern,
        ErrorType::ValidatorUnknownType,
        ErrorType::ValidatorInvalidScope,
        ErrorType::ValidatorInvalidShape,
        ErrorType::ValidatorDuplicateKey,
        ErrorType::ValidatorReservedAttribute,
        ErrorType::PatternMissingDescription,
    ];

    #[test]
    fn as_str_matches_serde_serialization() {
        for e in ALL {
            let via_serde = serde_json::to_value(e).unwrap();
            assert_eq!(
                via_serde.as_str().unwrap(),
                e.as_str(),
                "as_str drifted from serde for {e:?}"
            );
            // Display delegates to as_str.
            assert_eq!(e.to_string(), e.as_str());
        }
    }
}
