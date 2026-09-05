# Security Policy

*Security architecture, access control model, and responsible vulnerability disclosure.*

## Supported Versions

Only the latest release branch receives security patches:

| Version | Supported |
|---|---|
| `v1.0.x` (Current) | Yes |

## Access Control Model

Goshawk enforces a strict separation of concerns across three distinct on-chain identities:

| Role | Storage Type | Permissions |
|---|---|---|
| `owner` | Multi-sig (Gnosis Safe) / Hardware Wallet | Whitelist protocols and flash vaults, propose new executor, pause execution, sweep recovered tokens |
| `executor` | Server Hot EOA | Trigger `executeLiquidation()` on whitelisted venues, emergency sweep to `coldWallet` |
| `coldWallet` | Immutable Cold Address | Sole destination for automated profit sweeps |

### Compromise Containment

- **Compromised Executor:** The `executor` key cannot modify whitelists, alter the `coldWallet` address, or transfer funds to any destination other than `coldWallet`. Recovery is enacted via `owner.proposeExecutor()` followed by a 24-hour timelock execution.
- **Compromised Owner:** The `owner` cannot unilaterally sweep uncollateralized funds because no user deposits exist in the contract. Any proposed executor change emits `ExecutorProposed` with an immutable 24-hour delay.

## Attack Vector Mitigations

| Threat Vector | Mitigation |
|---|---|
| Same-Block Price Manipulation | Registration rejects any adapter with `OracleKind::SpotAmm` at boot. |
| Reentrancy Attacks | All entry points and callbacks are protected by `nonReentrant` mutex guards. |
| Arbitrary External Calls | All contract interactions require destination addresses to exist in `allowedProtocols` or `allowedFlashVaults`. |
| Residual Token Approvals | All DEX and pool allowances are explicitly reset to zero immediately after swap operations. |
| Mempool Front-Running | Transactions route through private builder channels (`Flashbots`, `Titan`, `Beaver`). |

## Reporting a Vulnerability

If you discover a security vulnerability in Goshawk, report it privately. Do not open public GitHub issues, discussions, or pull requests.

- **Email:** `security@goshawk.fi` (or via maintainer keybase / PGP)
- **Response Window:** Core maintainers acknowledge receipt within 24 hours and provide status updates every 48 hours until remediation.
- **Coordinated Disclosure:** We ask researchers to refrain from public disclosure until a patch has been verified and deployed on-chain.
