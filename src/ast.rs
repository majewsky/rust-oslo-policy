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

#[cfg(test)]
use std::fmt;

// NOTE: The types in here must be `pub` because peg::parser chokes if its output types are not
// `pub`. However, this entire module is `pub(crate)`, so these types do not actually appear in the
// public API.

/// A policy rule expression. This is the top-level type in the rule grammar.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Expression {
    Const(bool),
    Check(LeftHandSide, String),
    And(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    Not(Box<Expression>),
}

/// Helper for quickly comparing [Expression] objects in unit tests.
#[cfg(test)]
impl fmt::Display for Expression {
    ///Generates the expression's simplest representation in the policy language.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Expression::*;
        //`and` binds more strongly than `or`, so we need to use parentheses around an `or`
        //expression inside an `and` expression
        match self {
            Const(true) => f.write_str("@"),
            Const(false) => f.write_str("!"),
            Check(lhs, rhs) => write!(f, "{lhs}:{rhs}"),
            Not(e) => write!(f, "not {e}"),
            Or(lhs, rhs) => write!(f, "{lhs} or {rhs}"),
            And(lhs, rhs) => match (&**lhs, &**rhs) {
                (Or(_, _), Or(_, _)) => write!(f, "({lhs}) and ({rhs})"),
                (Or(_, _), _) => write!(f, "({lhs}) and {rhs}"),
                (_, Or(_, _)) => write!(f, "{lhs} and ({rhs})"),
                (_, _) => write!(f, "{lhs} and {rhs}"),
            },
        }
    }
}

/// Helper for quickly constructing [Expression] literals in unit tests.
#[cfg(test)]
impl From<bool> for Expression {
    fn from(x: bool) -> Expression {
        Expression::Const(x)
    }
}

/// The left-hand side of a check.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum LeftHandSide {
    Literal(String),
    Identifier(String),
}

/// Helper for quickly comparing [Expression] objects in unit tests.
#[cfg(test)]
impl fmt::Display for LeftHandSide {
    ///Generates the LHS's simplest representation in the policy language.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LeftHandSide::*;
        match self {
            Literal(s) => write!(f, "'{s}'"),
            Identifier(s) => f.write_str(s),
        }
    }
}

/// Helpers for quickly constructing [Expression] literals in unit tests.
#[cfg(test)]
pub mod build {
    use super::{Expression, LeftHandSide};

    pub fn make_check(l: impl Into<String>, r: impl Into<String>) -> Expression {
        Expression::Check(LeftHandSide::Identifier(l.into()), r.into())
    }
    pub fn make_literal_check(l: impl Into<String>, r: impl Into<String>) -> Expression {
        Expression::Check(LeftHandSide::Literal(l.into()), r.into())
    }
    pub fn make_and(l: impl Into<Expression>, r: impl Into<Expression>) -> Expression {
        Expression::And(Box::new(l.into()), Box::new(r.into()))
    }
    pub fn make_or(l: impl Into<Expression>, r: impl Into<Expression>) -> Expression {
        Expression::Or(Box::new(l.into()), Box::new(r.into()))
    }
    pub fn make_not(e: impl Into<Expression>) -> Expression {
        Expression::Not(Box::new(e.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;

    //This serialization is only used internally, but we want it to be correct because it is
    //used to write the parser test suite in a compact way.
    #[test]
    fn test_serialization() {
        //This test suite is inspired by the reference implementation's test suite.
        //<https://opendev.org/openstack/oslo.policy/src/commit/e7b9dd1f5ab10b447faba291ca0f89089aa46bcc/oslo_policy/tests/test_parser.py#L388-L403>
        let expr = make_and(true, false);
        assert_eq!(expr.to_string(), "@ and !");

        let expr = make_or(true, false);
        assert_eq!(expr.to_string(), "@ or !");

        let expr = make_or(true, make_or(false, make_not(true)));
        assert_eq!(expr.to_string(), "@ or ! or not @");
        let expr = make_or(make_or(true, false), make_not(true));
        assert_eq!(expr.to_string(), "@ or ! or not @");

        let expr = make_or(true, make_and(false, make_not(true)));
        assert_eq!(expr.to_string(), "@ or ! and not @");

        let expr = make_or(make_and(true, false), make_not(true));
        assert_eq!(expr.to_string(), "@ and ! or not @");

        let expr = make_and(true, make_and(false, make_not(true)));
        assert_eq!(expr.to_string(), "@ and ! and not @");
        let expr = make_and(make_and(true, false), make_not(true));
        assert_eq!(expr.to_string(), "@ and ! and not @");

        let expr = make_and(true, make_or(false, make_not(true)));
        assert_eq!(expr.to_string(), "@ and (! or not @)");

        let expr = make_and(make_or(true, false), make_not(true));
        assert_eq!(expr.to_string(), "(@ or !) and not @");
    }
}
