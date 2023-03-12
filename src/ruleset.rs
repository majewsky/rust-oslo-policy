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

use std::collections::HashMap;
use thiserror::Error;

use crate::ast::{Expression, LeftHandSide};
use crate::checkers::*;
use crate::parser::{parse_expression, InternalParseError};
use crate::request::{resolve_target_attr_refs, Request};

/// A container and evaluation engine for policy rules.
pub struct RuleSet {
    rules: HashMap<String, Expression>,
    checkers: HashMap<String, Box<dyn Checker>>,
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleSet {
    /// Returns a new empty RuleSet. The default set of [checkers][Checker] is registered
    /// automatically.
    pub fn new() -> Self {
        let mut rs = Self {
            rules: HashMap::new(),
            checkers: HashMap::new(),
        };
        rs.add_checker("rule", RuleChecker);
        rs.add_checker("role", RoleChecker);
        rs
    }

    /// Adds a custom checker to this RuleSet.
    pub fn add_checker(&mut self, name: impl Into<String>, check: impl Checker) {
        self.checkers.insert(name.into(), Box::new(check));
    }

    /// Parses a single rule and adds it to this RuleSet.
    pub fn add_rule(&mut self, name: impl Into<String>, expr: &str) -> Result<(), ParseError> {
        let name = name.into();
        match parse_expression(expr) {
            Ok(expr) => {
                self.rules.insert(name, expr);
                Ok(())
            }
            Err(err) => Err(ParseError {
                rule_name: name,
                error: err,
            }),
        }
    }

    /// Parses multiple rules and adds them to this RuleSet.
    pub fn add_rules(&mut self, rules: HashMap<String, String>) -> Result<(), ParseError> {
        for (name, rule_str) in rules {
            self.add_rule(name, &rule_str)?;
        }
        Ok(())
    }

    /// Evaluates the named rule for the given Request. If no rule with the given name exists,
    /// false is returned.
    pub fn evaluate(&self, rule_name: &str, req: &Request) -> bool {
        //TODO: add trace logging for rules
        match self.rules.get(rule_name) {
            Some(expr) => self.evaluate_expr(req, expr),
            None => false,
        }
    }

    fn evaluate_expr(&self, req: &Request, expr: &Expression) -> bool {
        use Expression::*;
        match expr {
            Const(val) => *val,
            Check(lhs, rhs) => self.evaluate_check(req, lhs, rhs),
            And(x, y) => self.evaluate_expr(req, x) && self.evaluate_expr(req, y),
            Or(x, y) => self.evaluate_expr(req, x) || self.evaluate_expr(req, y),
            Not(x) => !self.evaluate_expr(req, x),
        }
    }

    //TODO: add trace logging for checks
    fn evaluate_check(&self, req: &Request, lhs: &LeftHandSide, rhs: &str) -> bool {
        //expand %(foo)s syntax on the right-hand side
        let Some(rhs) = resolve_target_attr_refs(rhs, req.target) else {
            //If an interpolated variable is missing, the entire check fails.
            return false;
        };

        //option 1: LHS is a literal value
        use LeftHandSide::*;
        let lhs = match lhs {
            Literal(val) => return val == rhs,
            Identifier(id) => id,
        };

        //option 2: LHS is either a checker name or the name of an API attribute
        match self.checkers.get(lhs) {
            Some(checker) => checker.check(self, req, rhs),
            None => {
                let result = req.token.get_api_attribute(lhs).map(|val| val == rhs);
                //If the requested API attribute is missing, the entire check fails.
                result.unwrap_or(false)
            }
        }
    }
}

///Error type returned by [RuleSet::add_rule].
///
///This type hides the internal error type that the policy language parser returns.
#[derive(Error, Debug)]
#[error("could not parse rule {rule_name:?}: {error}")]
pub struct ParseError {
    rule_name: String,
    error: InternalParseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::test::Token;

    fn roles(names: &[&str]) -> Vec<String> {
        names.iter().map(|&n| n.to_owned()).collect()
    }
    fn pair(x: &str, y: &str) -> (String, String) {
        (x.to_owned(), y.to_owned())
    }

    #[test]
    fn test_ruleset_basic() {
        //This test scenario comes from:
        //<https://github.com/databus23/goslo.policy/blob/81bf2876dbdbdcaecc437bb2eb3549ea0e6b8490/policy_test.go#L11-L47>
        let token = Token {
            roles: roles(&["guest", "member"]),
            api_attrs: HashMap::from([pair("user_id", "u-1"), pair("project_id", "p-2")]),
        };
        let target = HashMap::from([
            pair("target.user_id", "u-1"),
            pair("user_id", "u-2"),
            pair("some_number", "1"),
            pair("some_bool", "True"),
        ]);
        let req = Request::new(&token).with_target(&target);

        let test_cases = [
            ("@", true),
            ("!", false),
            ("role:member", true),
            ("not role:member", false),
            ("role:admin", false),
            ("role:admin or role:guest", true),
            ("role:admin and role:guest", false),
            ("user_id:u-1", true),
            ("user_id:u-2", false),
            ("'u-2':%(user_id)s", true),
            //NOTE: We do not support non-string literals on the LHS. Putting them in quotes is
            //functionally identical to what the reference implementation does.
            ("'True':%(some_bool)s", true),
            ("'1':%(some_number)s", true),
            ("domain_id:%(does_not_exist)s", false),
            ("not (@ or @)", false),
            ("not @ or @", true),
            ("@ and (! or (not !))", true),
        ];
        for (rule_str, expected) in test_cases {
            let mut ruleset = RuleSet::new();
            ruleset.add_rule("test", rule_str).unwrap();
            let actual = ruleset.evaluate("test", &req);
            assert_eq!(actual, expected, "rule was: {rule_str}");
        }
    }

    #[test]
    fn test_realistic_roles() {
        //This test scenario comes from:
        //<https://github.com/databus23/goslo.policy/blob/81bf2876dbdbdcaecc437bb2eb3549ea0e6b8490/policy_test.go#L63-L117>

        let service_token = Token {
            roles: roles(&["service"]),
            api_attrs: HashMap::new(),
        };
        let service_req = Request::new(&service_token);

        let admin_token = Token {
            roles: roles(&["admin"]),
            api_attrs: HashMap::from([pair("domain_id", "admin_domain_id")]),
        };
        let admin_req = Request::new(&admin_token);

        let user_token = Token {
            roles: roles(&["member"]),
            api_attrs: HashMap::from([pair("user_id", "u-1")]),
        };
        let user_target1 = HashMap::from([pair("user_id", "u-1")]);
        let user_req1 = Request::new(&user_token).with_target(&user_target1);
        let user_target2 = HashMap::from([pair("user_id", "u-2")]);
        let user_req2 = Request::new(&user_token).with_target(&user_target2);

        let rules = HashMap::from([
            //This is an excerpt of the Keystone policy that the original test scenario uses.
            pair("admin_required", "role:admin"),
            pair(
                "cloud_admin",
                "rule:admin_required and domain_id:admin_domain_id",
            ),
            pair("service_role", "role:service"),
            pair(
                "service_or_admin",
                "rule:admin_required or rule:service_role",
            ),
            pair(
                "owner",
                "user_id:%(user_id)s or user_id:%(target.token.user_id)s",
            ),
            pair(
                "service_admin_or_owner",
                "rule:service_or_admin or rule:owner",
            ),
        ]);
        let mut ruleset = RuleSet::new();
        ruleset.add_rules(rules).unwrap();

        let test_cases = [
            (&service_req, "service_or_admin", true),
            (&service_req, "non_existent_rule", false),
            (&admin_req, "cloud_admin", true),
            (&admin_req, "service_admin_or_owner", true),
            (&user_req1, "service_admin_or_owner", true),
            (&user_req2, "service_admin_or_owner", false),
        ];
        for (req, rule_name, expected) in test_cases {
            let actual = ruleset.evaluate(rule_name, req);
            assert_eq!(actual, expected, "rule was: {rule_name}");
        }
    }
}
