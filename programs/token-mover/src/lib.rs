use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("3ugAnxv6g2zKEjyhWG94rRpBVT58QRgSBw8joBRWC2Up");

#[program]
pub mod token_mover {
    use super::*;

    /// Moves `amount` base units from `source_token` to `destination_token`
    /// by CPI into Token-2022's `transfer_checked`. The mint's transfer hook
    /// still runs: its accounts arrive as remaining accounts, hook program first.
    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        require!(
            !ctx.remaining_accounts.is_empty(),
            MoverError::MissingHookAccounts
        );

        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();
        let decimals = ctx.accounts.mint.decimals;

        // The hook program is whatever the caller put first. Token-2022 checks
        // it against the hook stored on the mint, so a wrong one just fails.
        let hook_program_id = ctx.remaining_accounts[0].key();

        // 1. Build a plain transfer_checked instruction.
        let mut ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            source.key,
            mint.key,
            destination.key,
            owner.key,
            &[],
            amount,
            decimals,
        )?;

        // 2. The account infos, in the same order as the instruction's accounts.
        let mut infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];

        // 3. Append the hook's accounts, read from the extra account meta list.
        add_extra_accounts_for_execute_cpi(
            &mut ix,
            &mut infos,
            &hook_program_id,
            source,
            mint,
            destination,
            owner,
            amount,
            ctx.remaining_accounts,
        )?;

        // 4. Invoke Token-2022. The owner signed this transaction, and that
        //    signature carries through the CPI.
        invoke(&ix, &infos)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,
    #[account(mut, token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[error_code]
pub enum MoverError {
    #[msg("Pass the hook program, its extra account meta list and its extra accounts as remaining accounts")]
    MissingHookAccounts,
}