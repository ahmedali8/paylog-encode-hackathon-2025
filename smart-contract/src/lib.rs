//! PayLog Attestation Registry (PLT-native, no shared account)
//! -----------------------------------------------------------
//! Problem:
//!   PLTs (Protocol-Level Tokens) are *account-based* and transfers are
//!   initiated/signed by the owner account. Contracts don't *hold* PLTs.
//!
//! Pattern:
//!   1) ORACLE (AI) verifies work off-chain and requests release on-chain.
//!   2) CLIENT sends PLT transfer (client -> freelancer) off-chain.
//!   3) CLIENT confirms payment on-chain with the PLT tx hash.
//!
//! This contract stores the attestation trail (work_hash, payment hash, timestamps)
//! and emits events for indexers / UI. It does NOT move tokens.

#![cfg_attr(not(feature = "std"), no_std)]

use concordium_std::*;

// ---- Aliases for clarity -----------------------------------------------------

/// Milestone identifiers are small integers for easy indexing.
type MilestoneId = u32;

/// 32-byte SHA-256 (work proof digest).
pub type Hash32 = [u8; 32];

/// 32-byte transaction hash (e.g., PLT transfer hash or registerData hash).
pub type TxHash = [u8; 32];

// ---- Init (constructor) parameters -------------------------------------------

/// Parameters passed at contract initialization time.
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct InitParams {
    /// UI-level identifier for the project (e.g., "wlog-local" or UUID).
    pub project_id: String,
    /// Client's account (payer).
    pub client: AccountAddress,
    /// Freelancer's account (payee).
    pub freelancer: AccountAddress,
    /// Oracle's account (AI agent / verifier).
    pub oracle: AccountAddress,
    /// Milestone amounts in *minor* units of the PLT (e.g., 6 decimals -> 100.00 = 100_000_000).
    pub amounts: Vec<u128>,
    /// For display purposes only (contract stores raw minor units).
    pub plt_decimals: u8,
}

// ---- Persistent state --------------------------------------------------------

/// Per-milestone state persisted on-chain.
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct Milestone {
    /// Agreed amount (minor units).
    pub amount_minor: u128,
    /// Has the oracle requested release (i.e., work verified)?
    pub requested: bool,
    /// Has the client confirmed payment (i.e., milestone released)?
    pub released: bool,
    /// Optional SHA-256 work hash provided by oracle.
    pub work_hash: Option<Hash32>,
    /// Optional PLT transfer tx hash provided by client at confirm.
    pub plt_tx_hash: Option<TxHash>,
    /// Timestamp at `requestRelease` (block time).
    pub requested_at_ms: Option<Timestamp>,
    /// Timestamp at `confirmPayment` (block time).
    pub attested_at_ms: Option<Timestamp>,
}

/// Contract storage (single-project instance).
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct State {
    pub project_id: String,         // for convenience in events/UI
    pub client: AccountAddress,     // payer (sole key holder)
    pub freelancer: AccountAddress, // payee
    pub oracle: AccountAddress,     // AI verifier
    pub plt_decimals: u8,           // display info
    pub milestones: Vec<Milestone>, // ordered milestones
}

// ---- Events (logged with enable_logger) --------------------------------------

/// Emitted when ORACLE requests release for a milestone.
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct ReleaseRequestedEvent {
    pub project_id: String,
    pub milestone_id: MilestoneId,
    pub work_hash: Hash32,
    pub requested_at_ms: Timestamp,
}

/// Emitted when CLIENT confirms payment (final attestation).
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct AttestedEvent {
    pub project_id: String,
    pub milestone_id: MilestoneId,
    pub work_hash: Hash32,
    pub plt_tx_hash: TxHash,
    pub amount_minor: u128,
    pub block_time_ms: Timestamp,
}

// ---- Errors ------------------------------------------------------------------

/// Errors for receive entrypoints (must implement `Reject`).
#[derive(Serial, Deserial, SchemaType, Debug, PartialEq, Eq, Reject)]
pub enum ContractError {
    Unauthorized,     // caller not allowed for this action
    InvalidMilestone, // out-of-bounds index
    AlreadyRequested, // request twice
    NotRequested,     // confirm without prior request
    AlreadyReleased,  // double-release attempt
    AmountMismatch,   // client-reported paid amount != configured
    LogError,         // failed to serialize/write event to chain log
    ParseError,       // failed to parse parameters
}

impl From<ParseError> for ContractError {
    fn from(_: ParseError) -> Self {
        ContractError::ParseError
    }
}

// ---- Init entrypoint ---------------------------------------------------------

/// Initialize state with participants and milestone amounts.
/// NOTE: No tokens move in this contract; PLT payments happen off-chain by accounts.
#[init(contract = "paylog", parameter = "InitParams")]
fn init(ctx: &InitContext, _sb: &mut StateBuilder) -> InitResult<State> {
    // Parse parameters (validated by schema).
    let p: InitParams = ctx.parameter_cursor().get()?;

    // Defensive: require at least one milestone.
    ensure!(!p.amounts.is_empty(), Reject::from(ParseError::default()));

    // Build milestones array from amounts.
    let ms = p
        .amounts
        .into_iter()
        .map(|amt| Milestone {
            amount_minor: amt,
            requested: false,
            released: false,
            work_hash: None,
            plt_tx_hash: None,
            requested_at_ms: None,
            attested_at_ms: None,
        })
        .collect::<Vec<_>>();

    Ok(State {
        project_id: p.project_id,
        client: p.client,
        freelancer: p.freelancer,
        oracle: p.oracle,
        plt_decimals: p.plt_decimals,
        milestones: ms,
    })
}

// ---- requestRelease (ORACLE -> verifies work) --------------------------------

/// Params for `requestRelease`.
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct RequestParam {
    pub milestone_id: MilestoneId, // which milestone is ready
    pub work_hash: Hash32,         // digest of normalized diff/artifact
}

/// Oracle-only: mark a milestone as ready-to-pay; store work hash & timestamp.
#[receive(
    contract = "paylog",
    name = "requestRelease",
    parameter = "RequestParam",
    error = "ContractError",
    mutable,
    enable_logger
)]
fn request_release(
    ctx: &ReceiveContext,   // call context (sender & metadata)
    host: &mut Host<State>, // mutable state handle
    logger: &mut Logger,    // event logger
) -> Result<(), ContractError> {
    // Only an account can call; contracts not allowed as oracle.
    let sender = match ctx.sender() {
        Address::Account(a) => a,
        _ => return Err(ContractError::Unauthorized),
    };
    // Enforce oracle-only access.
    ensure!(sender == host.state().oracle, ContractError::Unauthorized);

    // Parse params.
    let p: RequestParam = ctx.parameter_cursor().get()?;

    // Pull milestone (validate index).
    let ms = host
        .state_mut()
        .milestones
        .get_mut(p.milestone_id as usize)
        .ok_or(ContractError::InvalidMilestone)?;

    // Cannot request twice; also block post-release requests.
    ensure!(!ms.released, ContractError::AlreadyReleased);
    ensure!(!ms.requested, ContractError::AlreadyRequested);

    // Update state.
    ms.requested = true;
    ms.work_hash = Some(p.work_hash);
    ms.requested_at_ms = Some(ctx.metadata().block_time());

    // Emit ReleaseRequestedEvent for UI/indexers.
    let ev = ReleaseRequestedEvent {
        project_id: host.state().project_id.clone(),
        milestone_id: p.milestone_id,
        work_hash: p.work_hash,
        requested_at_ms: ctx.metadata().block_time(),
    };
    logger.log(&ev).map_err(|_| ContractError::LogError)?;

    Ok(())
}

// ---- confirmPayment (CLIENT -> after PLT transfer) ---------------------------

/// Params for `confirmPayment`.
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct ConfirmParam {
    pub milestone_id: MilestoneId, // must be previously requested
    pub paid_amount_minor: u128,   // sanity check
    pub plt_tx_hash: TxHash,       // 32-byte PLT transfer hash
}

/// Client-only: confirm the PLT payment and finalize attestation.
#[receive(
    contract = "paylog",
    name = "confirmPayment",
    parameter = "ConfirmParam",
    error = "ContractError",
    mutable,
    enable_logger
)]
fn confirm_payment(
    ctx: &ReceiveContext,
    host: &mut Host<State>,
    logger: &mut Logger,
) -> Result<(), ContractError> {
    // Only the client account can confirm.
    let sender = match ctx.sender() {
        Address::Account(a) => a,
        _ => return Err(ContractError::Unauthorized),
    };
    ensure!(sender == host.state().client, ContractError::Unauthorized);

    // Parse params.
    let p: ConfirmParam = ctx.parameter_cursor().get()?;

    // Get project_id before borrowing state_mut
    let project_id = host.state().project_id.clone();

    // Fetch milestone.
    let ms = host
        .state_mut()
        .milestones
        .get_mut(p.milestone_id as usize)
        .ok_or(ContractError::InvalidMilestone)?;

    // Must have been requested by the oracle, and not yet released.
    ensure!(ms.requested, ContractError::NotRequested);
    ensure!(!ms.released, ContractError::AlreadyReleased);

    // Optional: check the amount matches the configured budget.
    ensure!(
        p.paid_amount_minor == ms.amount_minor,
        ContractError::AmountMismatch
    );

    // Work hash must exist because requestRelease stored it.
    let work_hash = ms.work_hash.expect("work_hash set at request");
    let amount_minor = ms.amount_minor;

    // Finalize.
    ms.released = true;
    ms.plt_tx_hash = Some(p.plt_tx_hash);
    ms.attested_at_ms = Some(ctx.metadata().block_time());

    // Emit AttestedEvent.
    let ev = AttestedEvent {
        project_id,
        milestone_id: p.milestone_id,
        work_hash,
        plt_tx_hash: p.plt_tx_hash,
        amount_minor,
        block_time_ms: ctx.metadata().block_time(),
    };
    logger.log(&ev).map_err(|_| ContractError::LogError)?;

    Ok(())
}

// ---- Read-only view ----------------------------------------------------------

/// Input for `viewMilestone`.
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct ViewParam {
    pub milestone_id: MilestoneId,
}

/// Return model for `viewMilestone`.
#[derive(Serial, Deserial, SchemaType, Clone)]
pub struct MilestoneView {
    pub amount_minor: u128,
    pub requested: bool,
    pub released: bool,
    pub work_hash: Option<Hash32>,
    pub plt_tx_hash: Option<TxHash>,
    pub requested_at_ms: Option<Timestamp>,
    pub attested_at_ms: Option<Timestamp>,
}

/// Returns the milestone state (or `None` if out of range).
#[receive(
    contract = "paylog",
    name = "viewMilestone",
    parameter = "ViewParam",
    return_value = "Option<MilestoneView>"
)]
fn view_milestone(
    ctx: &ReceiveContext,
    host: &Host<State>,
) -> ReceiveResult<Option<MilestoneView>> {
    let p: ViewParam = ctx.parameter_cursor().get()?;
    let maybe = host.state().milestones.get(p.milestone_id as usize);
    Ok(maybe.map(|m| MilestoneView {
        amount_minor: m.amount_minor,
        requested: m.requested,
        released: m.released,
        work_hash: m.work_hash,
        plt_tx_hash: m.plt_tx_hash,
        requested_at_ms: m.requested_at_ms,
        attested_at_ms: m.attested_at_ms,
    }))
}
