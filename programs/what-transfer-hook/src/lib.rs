use std::{ cell::RefMut, str::FromStr };

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_2022::spl_token_2022::{
        extension::{
            transfer_hook::TransferHookAccount,
            BaseStateWithExtensionsMut,
            PodStateWithExtensionsMut,
        },
        pod::PodAccount,
    },
    token_interface::{ Mint, TokenAccount, TransferChecked, transfer_checked },
};
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta,
    seeds::Seed,
    state::ExtraAccountMetaList,
};
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

declare_id!("HSADtk7EsvDyMo55QA9fnDZubM31D1LTRrnp36itpydJ");

#[error_code]
pub enum TransferError {
    #[msg("The token is not currently transferring")]
    IsNotCurrentlyTransferring,

    #[msg("Amount Too big")]
    AmountTooBig,
}

#[program]
pub mod what_transfer_hook {
    use super::*;

    #[interface(spl_transfer_hook_interface::initialize_extra_account_meta_list)]
    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
        fee: u8,
        treasuryWallet: Pubkey
    ) -> Result<()> {
        // set authority field on white_list account as payer address
        ctx.accounts.white_list.authority = ctx.accounts.payer.key();
        ctx.accounts.white_list.is_on = true;
        ctx.accounts.white_list.fee = fee;
        ctx.accounts.white_list.treasury_wallet = treasuryWallet;

        // let extra_account_metas = InitializeExtraAccountMetaList::extra_account_metas()?;

        // initialize ExtraAccountMetaList account with extra accounts
        // ExtraAccountMetaList::init::<ExecuteInstruction>(
        //     &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
        //     &extra_account_metas
        // )?;
        Ok(())
    }

    #[interface(spl_transfer_hook_interface::execute)]
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        // Fail this instruction if it is not called from within a transfer hook
        check_is_transferring(&ctx)?;

        if ctx.accounts.white_list.is_on && !ctx.accounts.white_list.white_list.contains(&ctx.accounts.destination_token.key()) {
            panic!("Account not in white list!");
        }

        if amount > 50 {
            msg!("The amount is too big {0}", amount);
            // return err!(TransferError::AmountTooBig);
        }

        msg!("Owner {0}", ctx.accounts.owner.key());
        msg!("Destination {0}", ctx.accounts.destination_token.key());
        msg!("source mint {0}", ctx.accounts.source_token.key());


        let signer_seeds: &[&[&[u8]]] = &[&[b"delegate", &[ctx.bumps.delegate]]];

        // Transfer WSOL from sender to delegate token account using delegate PDA
        // transfer lamports amount equal to token transfer amount
        transfer_checked(
            CpiContext::new(ctx.accounts.token_program.to_account_info(), TransferChecked {
                from: ctx.accounts.sender_wsol_token_account.to_account_info(),
                mint: ctx.accounts.wsol_mint.to_account_info(),
                to: ctx.accounts.delegate_wsol_token_account.to_account_info(),
                authority: ctx.accounts.delegate.to_account_info(),
            }).with_signer(signer_seeds),
            amount,
            ctx.accounts.wsol_mint.decimals
        )?;

        Ok(())
    }

    pub fn add_to_whitelist(ctx: Context<AddToWhiteList>) -> Result<()> {
        if ctx.accounts.white_list.authority != ctx.accounts.signer.key() {
            panic!("Only the authority can add to the white list!");
        }

        ctx.accounts.white_list.white_list.push(ctx.accounts.new_account.key());
        msg!("New account white listed! {0}", ctx.accounts.new_account.key().to_string());
        msg!("White list length! {0}", ctx.accounts.white_list.white_list.len());

        Ok(())
    }

    pub fn turn_off_whitelist(ctx: Context<TurnOffWhitelist>) -> Result<()> {
        if ctx.accounts.white_list.authority != ctx.accounts.signer.key() {
            panic!("Only the authority can add to the white list!");
        }

        ctx.accounts.white_list.is_on = false;
        
        Ok(())
    }
}

fn check_is_transferring(ctx: &Context<TransferHook>) -> Result<()> {
    let source_token_info = ctx.accounts.source_token.to_account_info();
    let mut account_data_ref: RefMut<&mut [u8]> = source_token_info.try_borrow_mut_data()?;
    let mut account = PodStateWithExtensionsMut::<PodAccount>::unpack(*account_data_ref)?;
    let account_extension = account.get_extension_mut::<TransferHookAccount>()?;

    if !bool::from(account_extension.transferring) {
        return err!(TransferError::IsNotCurrentlyTransferring);
    }

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    payer: Signer<'info>,

    /// CHECK: ExtraAccountMetaList Account, must use these seeds
    #[account(
        init,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        space = ExtraAccountMetaList::size_of(
            InitializeExtraAccountMetaList::extra_account_metas()?.len()
        )?,
        payer = payer
    )]
    pub extra_account_meta_list: AccountInfo<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    #[account(
        init_if_needed, 
        seeds = [b"white_list"], 
        bump, 
        payer = payer, 
        space = 1000
    )]
    pub white_list: Account<'info, WhiteList>,
}

// Define extra account metas to store on extra_account_meta_list account
impl<'info> InitializeExtraAccountMetaList<'info> {
    pub fn extra_account_metas() -> Result<Vec<ExtraAccountMeta>> {
        // When the token2022 program CPIs to the transfer_hook instruction on this program,
        // the accounts are provided in order defined specified the list:

        // index 0-3 are the accounts required for token transfer (source, mint, destination, owner)
        // index 4 is address of ExtraAccountMetaList account
        Ok(
            vec![
                ExtraAccountMeta::new_with_seeds(
                    &[
                        Seed::Literal {
                            bytes: "white_list".as_bytes().to_vec(),
                        },
                    ],
                    false, // is_signer
                    true // is_writable
                )?,
                // index 6, wrapped SOL mint
                ExtraAccountMeta::new_with_pubkey(
                    &Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
                    false,
                    false
                )?,
                // index 7, token program (for wsol token transfer)
                ExtraAccountMeta::new_with_pubkey(&Token::id(), false, false)?,
                // index 8, associated token program
                ExtraAccountMeta::new_with_pubkey(&AssociatedToken::id(), false, false)?,
                // index 9, delegate PDA
                ExtraAccountMeta::new_with_seeds(
                    &[
                        Seed::Literal {
                            bytes: b"delegate".to_vec(),
                        },
                    ],
                    false, // is_signer
                    true // is_writable
                )?,
                // index 10, delegate wrapped SOL token account
                ExtraAccountMeta::new_external_pda_with_seeds(
                    8, // associated token program index
                    &[
                        Seed::AccountKey { index: 9 }, // owner index (delegate PDA)
                        Seed::AccountKey { index: 7 }, // token program index
                        Seed::AccountKey { index: 6 }, // wsol mint index
                    ],
                    false, // is_signer
                    true // is_writable
                )?,
                // index 11, sender wrapped SOL token account
                ExtraAccountMeta::new_external_pda_with_seeds(
                    8, // associated token program index
                    &[
                        Seed::AccountKey { index: 3 }, // owner index
                        Seed::AccountKey { index: 7 }, // token program index
                        Seed::AccountKey { index: 6 }, // wsol mint index
                    ],
                    false, // is_signer
                    true // is_writable
                )?,
            ]
        )
    }
}

// Order of accounts matters for this struct.
// The first 4 accounts are the accounts required for token transfer (source, mint, destination, owner)
// Remaining accounts are the extra accounts required from the ExtraAccountMetaList account
// These accounts are provided via CPI to this program from the token2022 program
#[derive(Accounts)]
pub struct TransferHook<'info> {
    #[account(token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(token::mint = mint)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: source token account owner, can be SystemAccount or PDA owned by another program
    pub owner: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList Account,
    #[account(seeds = [b"extra-account-metas", mint.key().as_ref()], bump)]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    #[account(seeds = [b"white_list"], bump)]
    pub white_list: Account<'info, WhiteList>,
    pub wsol_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(
        mut,
        seeds = [b"delegate"], 
        bump
    )]
    pub delegate: SystemAccount<'info>,
    #[account(
        mut,
        token::mint = wsol_mint, 
        token::authority = delegate,
    )]
    pub delegate_wsol_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        token::mint = wsol_mint, 
        token::authority = owner,
    )]
    pub sender_wsol_token_account: InterfaceAccount<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct AddToWhiteList<'info> {
    /// CHECK: New account to add to white list
    #[account()]
    pub new_account: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"white_list"],
        bump
    )]
    pub white_list: Account<'info, WhiteList>,
    #[account(mut)]
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct TurnOffWhitelist<'info> {
    #[account(
        mut,
        seeds = [b"white_list"],
        bump
    )]
    pub white_list: Account<'info, WhiteList>,
    #[account(mut)]
    pub signer: Signer<'info>,
}

#[account]
pub struct WhiteList {
    pub authority: Pubkey,
    pub initial_buyer: Pubkey,
    pub treasury_wallet: Pubkey,
    pub is_on: bool,
    pub fee: u8,
    pub white_list: Vec<Pubkey>,
}