use std::{ cell::RefMut };

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::spl_token_2022::{
        extension::{
            transfer_hook::TransferHookAccount,
            BaseStateWithExtensionsMut,
            PodStateWithExtensionsMut,
        },
        pod::PodAccount,
    },
    token_interface::{ Mint, TokenAccount },
};
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta,
    seeds::Seed,
    state::ExtraAccountMetaList,
};
use spl_transfer_hook_interface::instruction::{ExecuteInstruction, TransferHookInstruction};

declare_id!("A7TKxVmarz9XkuLB22xcjyKq8sLh3AAZZ8hbxxEixWw2");

#[error_code]
pub enum TransferError {
    #[msg("The token is not currently transferring")]
    IsNotCurrentlyTransferring,

    #[msg("Amount Too big")]
    AmountTooBig,

    #[msg("Numerical Overflow Error")]
    NumericalOverflow
}

#[program]
pub mod what_transfer_hook {
    use super::*;

    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        ctx.accounts.white_list.authority = ctx.accounts.payer.key();
        ctx.accounts.white_list.is_on = true;

        let extra_account_metas = InitializeExtraAccountMetaList::extra_account_metas()?;

        ExtraAccountMetaList::init::<ExecuteInstruction>(
            &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
            &extra_account_metas
        )?;
        Ok(())
    }

    #[interface(spl_transfer_hook_interface::execute)]
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        // Fail this instruction if it is not called from within a transfer hook
        check_is_transferring(&ctx)?;

        if ctx.accounts.white_list.is_on && !ctx.accounts.white_list.white_list.contains(&ctx.accounts.destination_token.key()) {
            panic!("Account not in white list!");
        }
        
        msg!("The amount is too big {0}", amount);
        msg!("Owner {0}", ctx.accounts.owner.key());
        msg!("Destination {0}", ctx.accounts.destination_token.key());
        msg!("source mint {0}", ctx.accounts.source_token.key());

        msg!("source mint amount {0}", ctx.accounts.source_token.amount);
        msg!("Destination mint amount {0}", ctx.accounts.destination_token.amount);

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

    pub fn fallback<'info>(
        program_id: &Pubkey,
        accounts: &'info [AccountInfo<'info>],
        data: &[u8],
    ) -> Result<()> {
        let instruction = TransferHookInstruction::unpack(data)?;

        // match instruction discriminator to transfer hook interface execute instruction  
        // token2022 program CPIs this instruction on token transfer
        match instruction {
            TransferHookInstruction::Execute { amount } => {
                let amount_bytes = amount.to_le_bytes();

                // invoke custom transfer hook instruction on our program
                __private::__global::transfer_hook(program_id, accounts, &amount_bytes)
            }
            _ => return Err(ProgramError::InvalidInstructionData.into()),
        }
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
    pub is_on: bool,
    pub white_list: Vec<Pubkey>,
}
