# WorkLog: On-Chain Attestation for PLT-Native Escrow (Concordium)

**Goal:** Record verifiable, on-chain attestations that a freelancer’s milestone was _paid_ in a PLT-native escrow model.  
**Important:** PLTs are protocol-level tokens and are **not** stored by smart contracts. This contract stores **attestations** only.

## 🔁 Execution flow (no shared account)

1. **Oracle (AI) verifies work** off-chain (e.g., Git diff → LLM).
   Calls `requestRelease(milestone_id, work_hash)` **from the ORACLE account**.
   Contract marks the milestone **requested** and logs `ReleaseRequestedEvent`.

2. **Client sends PLT** off-chain (token-holder op):
   Client transfers **PLT** from **client → freelancer** and keeps the **tx hash**.
   (Add an optional **memo** like `{"p":"wlog","m":1,"h":"<sha256>"}`; memos are up to **256 bytes**.)

3. **Client confirms on-chain**:
   Calls `confirmPayment(milestone_id, paid_amount_minor, plt_tx_hash)` **from the CLIENT account**.
   Contract checks requested flag & amount, marks **released**, stores the tx hash, and logs `AttestedEvent`.

Why this shape: PLTs are **protocol-level** and transfers are initiated by the owner account; contracts don’t hold PLTs. We therefore keep funds with the client and use an on-chain attestation handshake for auditability and automation.
