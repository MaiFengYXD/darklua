use std::{iter, ops};

use crate::nodes::{
    Block, Expression, FieldExpression, FunctionCall, InterpolatedStringExpression,
    InterpolationSegment, LocalAssignStatement, Prefix, StringExpression, TupleArguments,
    TypedIdentifier,
};
use crate::process::{IdentifierTracker, NodeProcessor, NodeVisitor, ScopeVisitor};
use crate::rules::{
    Context, FlawlessRule, RuleConfiguration, RuleConfigurationError, RuleProperties,
};

struct RemoveInterpolatedStringProcessor {
    string_format_identifier: String,
    define_string_format: bool,
    identifier_tracker: IdentifierTracker,
}

impl ops::Deref for RemoveInterpolatedStringProcessor {
    type Target = IdentifierTracker;

    fn deref(&self) -> &Self::Target {
        &self.identifier_tracker
    }
}

impl ops::DerefMut for RemoveInterpolatedStringProcessor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.identifier_tracker
    }
}

const DEFAULT_STRING_LIBRARY: &str = "string";
const DEFAULT_STRING_FORMAT_NAME: &str = "format";

impl RemoveInterpolatedStringProcessor {
    fn new(string_format_identifier: impl Into<String>) -> Self {
        Self {
            string_format_identifier: string_format_identifier.into(),
            define_string_format: false,
            identifier_tracker: Default::default(),
        }
    }

    fn replace_with(&mut self, string: &InterpolatedStringExpression) -> Expression {
        if string.is_empty() {
            StringExpression::from_value("").into()
        } else {
            self.define_string_format = true;

            let format_string = string.iter_segments().fold(
                String::new(),
                |mut format_string, segment| {
                    match segment {
                        InterpolationSegment::String(string_segment) => {
                            format_string
                                .push_str(&string_segment.get_value().replace('%', "%%"));
                        }
                        InterpolationSegment::Value(_) => {
                            format_string.push_str("%*");
                        }
                    }
                    format_string
                },
            );

            let arguments = iter::once(StringExpression::from_value(format_string).into())
                .chain(
                    string
                        .iter_segments()
                        .filter_map(|segment| match segment {
                            InterpolationSegment::Value(segment) => {
                                Some(segment.get_expression().clone())
                            }
                            InterpolationSegment::String(_) => None,
                        })
                )
                .collect::<TupleArguments>();

            FunctionCall::from_prefix(Prefix::from_name(&self.string_format_identifier))
                .with_arguments(arguments)
                .into()
        }
    }
}

impl NodeProcessor for RemoveInterpolatedStringProcessor {
    fn process_expression(&mut self, expression: &mut Expression) {
        if let Expression::InterpolatedString(string) = expression {
            *expression = self.replace_with(string);
        }
    }
}

pub const REMOVE_INTERPOLATED_STRING_RULE_NAME: &str = "remove_interpolated_string";

/// A rule that removes interpolated strings.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RemoveInterpolatedString;

impl FlawlessRule for RemoveInterpolatedString {
    fn flawless_process(&self, block: &mut Block, _: &Context) {
        const STRING_FORMAT_IDENTIFIER: &str = "__DARKLUA_STR_FMT";

        let mut processor = RemoveInterpolatedStringProcessor::new(STRING_FORMAT_IDENTIFIER);
        ScopeVisitor::visit_block(block, &mut processor);

        if processor.define_string_format {
            block.insert_statement(
                0,
                LocalAssignStatement::new(
                    vec![TypedIdentifier::new(STRING_FORMAT_IDENTIFIER)],
                    vec![FieldExpression::new(
                        Prefix::from_name(DEFAULT_STRING_LIBRARY),
                        DEFAULT_STRING_FORMAT_NAME,
                    )
                    .into()],
                ),
            );
        }
    }
}

impl RuleConfiguration for RemoveInterpolatedString {
    fn configure(&mut self, properties: RuleProperties) -> Result<(), RuleConfigurationError> {
        for (key, _) in properties {
            return Err(RuleConfigurationError::UnexpectedProperty(key));
        }

        Ok(())
    }

    fn get_name(&self) -> &'static str {
        REMOVE_INTERPOLATED_STRING_RULE_NAME
    }

    fn serialize_to_properties(&self) -> RuleProperties {
        RuleProperties::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rules::Rule;

    use insta::assert_json_snapshot;

    fn new_rule() -> RemoveInterpolatedString {
        RemoveInterpolatedString::default()
    }

    #[test]
    fn serialize_default_rule() {
        let rule: Box<dyn Rule> = Box::new(new_rule());

        assert_json_snapshot!("default_remove_interpolated_string", rule);
    }

    #[test]
    fn configure_with_extra_field_error() {
        let result = json5::from_str::<Box<dyn Rule>>(
            r#"{
            rule: 'remove_interpolated_string',
            prop: "something",
        }"#,
        );
        pretty_assertions::assert_eq!(result.unwrap_err().to_string(), "unexpected field 'prop'");
    }
}
