use anchor_lang::prelude::*;
use anchor_spl::token::{TokenAccount, Mint};
use crate::states::governance::GovernancePool;
use crate::states::citizen::Citizen;
use crate::states::citizen_index::CitizenIndex;
use crate::states::invite::GovernanceInvite;
use crate::error::GovernanceError;

#[derive(Accounts)]
#[instruction(name: String, region: u8, age_group: u8, other_demographic: u8)]
pub struct UseGovernanceInvite<'info> {
    #[account(mut)]
    pub governance_pool: Account<'info, GovernancePool>,

    #[account(
        mut,
        constraint = invite.governance_pool == governance_pool.key() @ GovernanceError::InvalidInvite,
        constraint = !invite.is_used @ GovernanceError::InviteAlreadyUsed,
        constraint = Clock::get()?.unix_timestamp <= invite.expires_at @ GovernanceError::InviteExpired,
    )]
    pub invite: Account<'info, GovernanceInvite>,

    #[account(
        init,
        payer = new_member,
        space = Citizen::SPACE,
        seeds = [b"citizen", governance_pool.key().as_ref(), new_member.key().as_ref()],
        bump
    )]
    pub citizen_account: Account<'info, Citizen>,

    #[account(mut)]
    pub new_member: Signer<'info>,

    #[account(
        mut,
        seeds = [b"citizen_index", governance_pool.key().as_ref(), &(governance_pool.total_citizens / CitizenIndex::MAX_CITIZENS_PER_INDEX as u32).to_le_bytes()],
        bump
    )]
    pub citizen_index: Account<'info, CitizenIndex>,

    #[account(
        associated_token::mint = governance_token_mint,
        associated_token::authority = new_member,
    )]
    pub member_token_account: Account<'info, TokenAccount>,

    pub governance_token_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn use_governance_invite(
    ctx: Context<UseGovernanceInvite>,
    name: String,
    region: u8,
    age_group: u8,
    other_demographic: u8
) -> Result<()> {
    let governance_pool = &mut ctx.accounts.governance_pool;
    let invite = &mut ctx.accounts.invite;
    let citizen_account = &mut ctx.accounts.citizen_account;
    let citizen_index = &mut ctx.accounts.citizen_index;
    let new_member = &ctx.accounts.new_member;
    let member_token_account = &ctx.accounts.member_token_account;

    // Validate input
    require!(name.len() <= Citizen::MAX_NAME_LENGTH, GovernanceError::InvalidInput);
    require!(region < 8, GovernanceError::InvalidDemographic);
    require!(age_group < 5, GovernanceError::InvalidDemographic);
    require!(other_demographic < 4, GovernanceError::InvalidDemographic);

    // Initialize citizen account
    citizen_account.name = name;
    citizen_account.governance_pool = governance_pool.key();
    citizen_account.is_eligible = true;
    citizen_account.last_participation = 0;
    citizen_account.region = region;
    citizen_account.age_group = age_group;
    citizen_account.other_demographic = other_demographic;
    citizen_account.is_initialized = true;

    // Initialize citizen index if it's new
    if citizen_index.governance_pool == Pubkey::default() {
        citizen_index.governance_pool = governance_pool.key();
        citizen_index.citizens = Vec::new();
        citizen_index.count = 0;
    }

    // Add citizen to the index
    if citizen_index.citizens.len() >= CitizenIndex::MAX_CITIZENS_PER_INDEX {
        return Err(GovernanceError::CitizenIndexFull.into());
    }
    citizen_index.citizens.push(new_member.key());
    citizen_index.count += 1;

    // Update governance pool
    governance_pool.total_citizens += 1;

    // Check if we need to create a new index
    if governance_pool.total_citizens % CitizenIndex::MAX_CITIZENS_PER_INDEX as u32 == 0 {
        governance_pool.total_citizen_indices += 1;
    }

    // Mark the invite as used
    invite.is_used = true;
    invite.used_by = Some(new_member.key());

    emit!(CitizenAddedToGovernance {
        governance_pool: governance_pool.key(),
        citizen: new_member.key(),
        token_amount: member_token_account.amount,
    });

    Ok(())
}

#[event]
pub struct CitizenAddedToGovernance {
    pub governance_pool: Pubkey,
    pub citizen: Pubkey,
    pub token_amount: u64,
}