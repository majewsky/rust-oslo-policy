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

use crate::request::Request;
use crate::ruleset::RuleSet;

/// Generic interface for policy building blocks.
///
/// In a policy expression, each non-whitespace sequence of characters with a colon in them is a
/// **check**. For example, `rule:compute:get_all` is a check with the left-hand side `rule` and the
/// right-hand side `compute:get_all`.
///
/// Checks need to be registered with the [RuleSet] in order to get used while a policy is being
/// enforced.
pub trait Checker: Send + Sync + 'static {
    /// Execute a check. If this checker is registered with a [RuleSet], this method will be called
    /// during policy evaluation upon encountering a check whose left-hand side is equal to the
    /// rule's registered name. The right-hand side of the check is supplied in the `rhs` argument.
    /// The Checker can also inspect the [Request] that was made by the user.
    fn check(&self, ruleset: &RuleSet, req: &Request, rhs: &str) -> bool;
}

/// A [Checker] that matches if the user has a certain role.
///
/// For example, the check `role:foo` will return whether the token presented by the user covers
/// the role named `foo`.
///
/// By default, this check is registered under the name "role".
pub struct RoleChecker;

impl Checker for RoleChecker {
    fn check(&self, _ruleset: &RuleSet, req: &Request, rhs: &str) -> bool {
        req.token.has_role(rhs)
    }
}

/// A [Checker] that recurses into a different rule.
///
/// For example, the check `rule:foo` will return the result of evaluating the rule `foo`, or false
/// if no rule with that name exists in the [RuleSet].
///
/// By default, this check is registered under the name "rule".
pub struct RuleChecker;

impl Checker for RuleChecker {
    fn check(&self, ruleset: &RuleSet, req: &Request, rhs: &str) -> bool {
        ruleset.evaluate(rhs, req)
    }
}
