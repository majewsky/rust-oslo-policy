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

/// Attributes belonging to a single request.
pub struct Request<'a> {
    /// Attributes associated with a token that was supplied by the user for this request.
    pub token: &'a dyn Token,
    /// Attributes associated with the target object(s) of this request.
    pub target: &'a dyn Target,
}

impl<'a> Request<'a> {
    /// Create a new request. The `token` argument describes the token that was supplied by the
    /// user as part of the current request, see documentation on [the Token trait][Token].
    ///
    /// The `api_attributes` can appear on the left side of a generic check. For example,
    /// `project_name:cloud_admin` checks whether the API attribute `project_name` has the string
    /// value `cloud_admin`.
    ///
    /// API attributes are usually derived from the validated token that was supplied by the user.
    pub fn new(token: &'a dyn Token) -> Self {
        Request { token, target: &() }
    }

    /// Add a [Target] to this request. This is usually chained directly after [Request::new].
    pub fn with_target(mut self, target: &'a dyn Target) -> Self {
        self.target = target;
        self
    }
}

/// Attributes associated with a token that was supplied by the user as part of a [Request].
pub trait Token {
    /// Returns the API attribute with the given `name`, if it exists. API attributes can appear on
    /// the left side of a check. For example, `project_name:cloud_admin` checks whether the API
    /// attribute `project_name` exists and has the string value `cloud_admin`.
    #[allow(clippy::needless_lifetimes)] //false positive
    fn get_api_attribute<'k>(&self, name: &'k str) -> Option<&str>;

    /// Returns whether this token covers the given role.
    fn has_role(&self, role_name: &str) -> bool;
}

#[cfg(test)]
pub(crate) mod test {
    use std::collections::HashMap;

    /// A simple implementor of the Token trait for use in tests.
    pub struct Token {
        pub roles: Vec<String>,
        pub api_attrs: HashMap<String, String>,
    }

    impl super::Token for Token {
        #[allow(clippy::needless_lifetimes)] //false positive
        fn get_api_attribute<'k>(&self, name: &'k str) -> Option<&str> {
            self.api_attrs.get(name).map(|x| &**x)
        }
        fn has_role(&self, role_name: &str) -> bool {
            self.roles.iter().any(|n| n == role_name)
        }
    }
}

/// Attributes associated with the target object of a [Request].
///
/// This covers attributes that were supplied by the user within the request payload, specifically
/// IDs or names appearing within the request path or the request body.
///
/// This crate defines two basic implementations: A `HashMap<String, String>` can be used if the
/// caller does not want to define a specific trait implementor type. If a request does not have
/// any target attributes, the unit value `()` can be used as a Target to avoid allocating an empty
/// HashMap.
pub trait Target {
    /// Returns the target object attribute with the given `name`, if it exists.
    #[allow(clippy::needless_lifetimes)] //false positive
    fn get_attribute<'n>(&self, name: &'n str) -> Option<&str>;
}

impl Target for () {
    /// Always returns `None`, since this target does not have any attributes.
    #[allow(clippy::needless_lifetimes)] //false positive
    fn get_attribute<'n>(&self, _name: &'n str) -> Option<&str> {
        None
    }
}

impl Target for HashMap<String, String> {
    #[allow(clippy::needless_lifetimes)] //false positive
    fn get_attribute<'n>(&self, name: &'n str) -> Option<&str> {
        self.get(name).map(|s| s.as_ref())
    }
}

/// Resolves references to target object attributes in the `%(foo)s` syntax on the right-hand side
/// of a check.
pub(crate) fn resolve_target_attr_refs<'r, 'i: 'r, 't: 'r>(
    input: &'i str,
    target: &'t dyn Target,
) -> Option<&'r str> {
    //We currently only support exactly one %(foo)s interpolation that spans the entire string.
    //Otherwise we return the input unchanged.
    let Some(stripped) = input.strip_prefix("%(") else {
        return Some(input);
    };
    let Some(attr_name) = stripped.strip_suffix(")s") else {
        return Some(input);
    };
    target.get_attribute(attr_name)
}
