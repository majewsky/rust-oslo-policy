/******************************************************************************
*
*  Copyright 2023 Stefan Majewsky <majewsky@gmx.net>
*
*  Licensed under the Apache License, Version 2.0 (the "License");
*  you may not use this file except in compliance with the License.
*  You may obtain a copy of the License at
*
*      http://www.apache.org/licenses/LICENSE-2.0
*
*  Unless required by applicable law or agreed to in writing, software
*  distributed under the License is distributed on an "AS IS" BASIS,
*  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
*  See the License for the specific language governing permissions and
*  limitations under the License.
*
******************************************************************************/

use crate::ast::*;

peg::parser! {
    grammar policy_parser() for str {
        rule _ = [' ' | '\t' | '\n']*

        pub rule expr() -> Expression
            = quiet!{_ e:expr_inner() _ { e }}
            / expected!("expression")

        rule expr_inner() -> Expression = precedence!{
            x:(@) _ "or" _ y:@ { Expression::Or(Box::new(x), Box::new(y)) }
            --
            x:(@) _ "and" _ y:@ { Expression::And(Box::new(x), Box::new(y)) }
            --
            "not" e:@ { Expression::Not(Box::new(e)) }
            e:atom() { e }
        }

        rule atom() -> Expression
            = quiet!{_ c:atom_inner() _ { c }}
            / expected!("check or opening parenthesis")

        rule atom_inner() -> Expression
            = "@" { Expression::Const(true) }
            / "!" { Expression::Const(false) }
            / "(" e:expr() ")" { e }
            / l:check_lhs() ":" r:check_rhs() { Expression::Check(l, r) }

        // The way that the reference implementation parses checks is completely insane. It
        // tokenizes by splitting on whitespace and recognizes trailing "(" and leading ")" on each
        // token. Then whatever is left is taken as EXACTLY one logic operator keyword (and/or/not)
        // or check. From this follows that checks cannot contain whitespace, even if they contain
        // quoted strings. The rule 'foo':%(bar)s works as expected, but 'foo foo':%(bar)s does
        // not, because it gets split on the whitespace between the foo's, even though it looks
        // like the quoting would prevent token splitting here. We mimic this insane behavior here.
        rule check_lhs() -> LeftHandSide
            = "'"  s:check_lhs_inner() "'"  { LeftHandSide::Literal(s) }
            / "\"" s:check_lhs_inner() "\"" { LeftHandSide::Literal(s) }
            /      s:check_lhs_inner()      { LeftHandSide::Identifier(s) }
        // We forbid:
        // - whitespace in any part of the check (as explained above)
        // - colons on the LHS (the first colon in the check splits LHS and RHS)
        // - quotes and backslashes on the LHS (we only support simple string literals without
        //   escape sequences)
        rule check_lhs_inner() -> String
            = s:$([^' ' | '\t' | '\n' | ':' | '\'' | '"' | '\\']+) { s.to_owned() }
        rule check_rhs() -> String
            = s:$([^' ' | '\t' | '\n']+) { s.to_owned() }
    }
}

// The policy_parser module is private, so we need to expose an explicit interface to the outside.
pub(crate) type InternalParseError = peg::error::ParseError<peg::str::LineCol>;
pub(crate) fn parse_expression(input: &str) -> Result<Expression, InternalParseError> {
    policy_parser::expr(input)
}

#[cfg(test)]
mod tests {
    use super::parse_expression;
    use crate::ast::build::*;

    //Several of these tests are adapted from the reference implementation's test suite.

    #[test]
    fn test_basic() {
        assert_eq!(parse_expression("@ and !"), Ok(make_and(true, false)));
        assert_eq!(
            parse_expression("    @    or   !  "),
            Ok(make_or(true, false))
        );
    }

    fn assert_all_identical(inputs: &[&'static str]) {
        let expr0 = parse_expression(inputs[0]);
        for input in inputs.iter() {
            let expr = parse_expression(input);
            assert_eq!(
                expr, expr0,
                "left input was {:?}, right input was {:?}",
                input, inputs[0]
            );
        }
    }

    #[test]
    fn test_all_identical() {
        //<https://opendev.org/openstack/oslo.policy/src/commit/e7b9dd1f5ab10b447faba291ca0f89089aa46bcc/oslo_policy/tests/test_parser.py#L449-L459>
        assert_all_identical(&[
            "( @ ) and ! or @",
            "@ and ( ! ) or @",
            "@ and ! or ( @ )",
            "( @ ) and ! or ( @ )",
            "@ and ( ! ) or ( @ )",
            "( @ ) and ( ! ) or ( @ )",
            "( @ and ! ) or @",
            "( ( @ ) and ! ) or @",
            "( @ and ( ! ) ) or @",
            "( ( @ and ! ) ) or @",
            "( @ and ! or @ )",
        ]);
        //<https://opendev.org/openstack/oslo.policy/src/commit/e7b9dd1f5ab10b447faba291ca0f89089aa46bcc/oslo_policy/tests/test_parser.py#L468-L473>
        assert_all_identical(&[
            "not ( @ ) and ! or @",
            "not @ and ( ! ) or @",
            "not @ and ! or ( @ )",
            "( not @ ) and ! or @",
            "( not @ and ! ) or @",
            "( not @ and ! or @ )",
        ]);
        //<https://opendev.org/openstack/oslo.policy/src/commit/e7b9dd1f5ab10b447faba291ca0f89089aa46bcc/oslo_policy/tests/test_parser.py#L486-L491>
        assert_all_identical(&[
            "( @ ) and not ! or @",
            "@ and ( not ! ) or @",
            "@ and not ( ! ) or @",
            "@ and not ! or ( @ )",
            "( @ and not ! ) or @",
            "( @ and not ! or @ )",
        ]);
        //<https://opendev.org/openstack/oslo.policy/src/commit/e7b9dd1f5ab10b447faba291ca0f89089aa46bcc/oslo_policy/tests/test_parser.py#L504-L509>
        assert_all_identical(&[
            "( @ ) and ! or not @",
            "@ and ( ! ) or not @",
            "@ and ! or not ( @ )",
            "@ and ! or ( not @ )",
            "( @ and ! ) or not @",
            "( @ and ! or not @ )",
        ]);
    }

    #[test]
    fn test_parsing_of_checks() {
        //test success cases
        let input = "user_id:%(target.user_id)s and role:compute:get_all";
        let lhs = make_check("user_id", "%(target.user_id)s");
        let rhs = make_check("role", "compute:get_all");
        let parsed = parse_expression(input);
        assert_eq!(parsed, Ok(make_and(lhs, rhs)));
        assert_eq!(parsed.unwrap().to_string(), input);

        //test more success cases
        let input = "is_admin:True or 'Member':%(role.name)s";
        let lhs = make_check("is_admin", "True");
        let rhs = make_literal_check("Member", "%(role.name)s");
        let parsed = parse_expression(input);
        assert_eq!(parsed, Ok(make_or(lhs, rhs)));
        assert_eq!(parsed.unwrap().to_string(), input);

        //test more success cases
        let input = "\"Member\":%(role.name)s";
        let check = make_literal_check("Member", "%(role.name)s");
        let parsed = parse_expression(input);
        assert_eq!(parsed, Ok(check));
        //This does not roundtrip back into `input` because serialization uses single quotes.
        assert_eq!(parsed.unwrap().to_string(), "'Member':%(role.name)s");

        //test failure case: empty string is not a valid rule (this is an intentional deviation
        //from the default implementation, where empty string means "allow all")
        let input = "";
        assert!(parse_expression(input).is_err());

        //test failure case: whitespace in check is not allowed (compatibility with ref.impl.)
        let input = "'foo bar':%(role.name)s";
        assert!(parse_expression(input).is_err());

        //test failure case: escape sequences in literals are not allowed (not needed so far)
        for escape_sequence in ["\\n", "\\\\", "\\\"", "\\'"] {
            let input = format!("'foo{escape_sequence}bar':%(role.name)s");
            assert!(parse_expression(&input).is_err());
        }
    }
}
