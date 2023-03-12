# oslo-policy

A parser and evaluation engine for [oslo.policy][ref-docs] rule files.

This crate may be interesting for you if you are implementing an OpenStack-like service in Rust. If
your service does not have anything to do with OpenStack, there are better policy engines to choose
from. This engine is designed for the highest reasonable compatibility with the OpenStack way.

## Usage

Policy rules are usually stored in a YAML or JSON file on disk. Load that file into a HashMap using
your IO and deserialization libraries of choice. (The example below uses std and serde\_yaml.) Then
use this library to parse the rules into a RuleSet object:

```rust,ignore
let buf = std::fs::read("/etc/myservice/policy.yaml")?;
let rules = serde_yaml::from_bytes(&buf)?;

let mut ruleset = oslo_policy::RuleSet::new();
ruleset.add_rules(rules)?;
```

When handling a request, you need to construct a Request object. At a minimum, a request needs to
contain a Token object that describes the token which was supplied with the Request. Ideally, your
OpenStack client library of choice should have a type that implements our Token trait. Once you have
a Request object, you can evaluate policy rules from the RuleSet and generate your HTTP responses
accordingly. (The example below implies that Hyper is used to implement the request handler.)

```rust,ignore
use hyper::{Body, Request, Response, Server};

// in request handler:
let req = oslo_policy::Request::new(&token);
if !ruleset.evaluate("instance:create", &req) {
  return Err(Response::builder().status(403).body("Forbidden").unwrap());
}

```

## Differences to the reference implementation

This library does not replicate all of the features and behaviors of the
[reference implementation][ref-impl].

### Intentional incompatibilities

This library explicitly rejects some fallback behaviors of the reference implementation that we
consider dangerous.

- In the reference implementation, the empty string is a valid rule which means "accept all". This
  is a surprising behavior in a place where surprises are a bad thing, so this library rejects empty
  rule strings with a parse error instead. If you want "accept all", write `@` explicitly.
- The reference implementation supports designating a rule as the default rule. Why is this
  dangerous: When new API endpoints are added in a new version of a service, and the operator does
  not update the policy to match the new version, the new endpoints will be subject to the default
  rule, however conservative or liberal that rule may be. This library takes the safe default and
  returns false when there is no rule defined under the requested name.

### Intentionally out of scope

The following functionality will never be implemented in this library. PRs that add these features
will be rejected.

- The reference implementation supports an alternative rule syntax wherein rules are not encoded in
  string format like `"(rule:foo and rule:bar) or rule:qux"`, but in a list of lists of strings like
  `[["rule:foo", "rule:bar"], ["rule:qux"]]`. This format is described as legacy in the reference
  implementation and not in wide use anymore. Always use the string format instead.
- The reference implementation supports automatically reloading rule files when they change on disk.
  This implementation does not support this out of the box, because doing so introduces a lot of
  complexity. Most people will not want this extra complexity since they restart on changed
  configuration anyway (esp. when running in a containerized environment).
- The reference implementation allows arbitrary Python literal expressions on the left side of a
  check (as long as no whitespace or colons are used), e.g. `["foo","bar"]:%(role_name)s` is
  technically a valid check, though it does not do what you expect. It actually checks if the target
  object attribute `role_name` exists and contains the string value `['foo', 'bar']`, since the
  left-hand side of the check is parsed and then serialized again through Python's `str()` operator.
  Supporting all of that is clearly insane and we are not going to do it. We only support checks
  with plain string literals, e.g. `'foo':%(name)s` or `"foo":%(name)s`.
- The reference implementation offers a pre-defined checker called `http` for delegating a policy
  decision to a different service that is reachable via HTTP. Implementing such a checker is out of
  scope for this library. If it is needed, applications can use `Enforcer::add_check` to register a
  custom implementation of `trait Checker`.

### Currently out of scope

The following functionality may be implemented in this library in the future if a practical usecase
can be demonstrated. Please open an issue to discuss your usecase before sending a PR.

- Our parsing of string literals on the left-hand side of a check does not support escape sequences.
  The reference implementation allows `'foo\nbar':%(name)s` or `"foo\"bar":%(name)s`, but this
  library currently returns a parse error when encountering an escape sequence in a string literal.
- When interpolating target object attributes on the right-hand side of a check, we only support
  a single string interpolation that covers the entire right-hand side. For example,
  `"foo":%(name)s` works, but `42:%(count)d` and `"foo_bar":%(id)s_%(name)s` do not work and always
  yield false.

[ref-docs]: https://docs.openstack.org/oslo.policy/latest/
[ref-impl]: https://opendev.org/openstack/oslo.policy/
