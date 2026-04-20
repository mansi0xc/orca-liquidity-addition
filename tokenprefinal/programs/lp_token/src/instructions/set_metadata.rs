use crate::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};
use anchor_spl::token::Mint;

/// Metaplex Token Metadata program ID (mainnet/devnet).
pub const TOKEN_METADATA_PROGRAM_ID: Pubkey =
    anchor_lang::solana_program::pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

/// Create or update on-chain token metadata via Metaplex Token Metadata program.
///
/// EVM equivalent: name() and symbol() are set during initialize().
/// On Solana, SPL mints do not store name/symbol natively — Metaplex metadata
/// is the standard way to associate human-readable info with a mint.
///
/// Access control: only the token owner may call this.
/// The token_state PDA is both the mint_authority and the metadata update_authority,
/// allowing it to sign the CPI.
#[derive(Accounts)]
pub struct SetMetadata<'info> {
    /// Must be the registered owner.
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Program governance state — validates owner.
    #[account(
        seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()],
        bump = token_state.bump,
        constraint = owner.key() == token_state.owner @ LPTokenError::Unauthorized,
    )]
    pub token_state: Account<'info, TokenState>,

    /// The SPL mint to attach metadata to.
    pub token_mint: Account<'info, Mint>,

    /// The metadata account (PDA derived by Metaplex program).
    /// Seeds: ["metadata", token_metadata_program_id, mint_pubkey]
    /// CHECK: Account is created/validated by the Metaplex program via CPI.
    /// We verify the correct PDA derivation via seeds constraint.
    #[account(
        mut,
        seeds = [
            b"metadata",
            TOKEN_METADATA_PROGRAM_ID.as_ref(),
            token_mint.key().as_ref(),
        ],
        seeds::program = TOKEN_METADATA_PROGRAM_ID,
        bump,
    )]
    pub metadata: UncheckedAccount<'info>,

    /// The Metaplex Token Metadata program.
    /// CHECK: Verified by address constraint.
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub token_metadata_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Borsh-serialized DataV2 for Metaplex CPI.
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
struct DataV2 {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub seller_fee_basis_points: u16,
    pub creators: Option<Vec<Creator>>,
    pub collection: Option<Collection>,
    pub uses: Option<Uses>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
struct Creator {
    pub address: Pubkey,
    pub verified: bool,
    pub share: u8,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
struct Collection {
    pub verified: bool,
    pub key: Pubkey,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
struct Uses {
    pub use_method: u8,
    pub remaining: u64,
    pub total: u64,
}

impl SetMetadata<'_> {
    pub fn apply(ctx: &mut Context<SetMetadata>, params: &SetMetadataParams) -> Result<()> {
        let mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[
            TOKEN_STATE_SEED,
            mint_key.as_ref(),
            &[ctx.accounts.token_state.bump],
        ];
        let signer_seeds: &[&[&[u8]]] = &[seeds];

        let data = DataV2 {
            name: params.name.clone(),
            symbol: params.symbol.clone(),
            uri: params.uri.clone(),
            seller_fee_basis_points: 0,
            creators: None,
            collection: None,
            uses: None,
        };

        let metadata_info = ctx.accounts.metadata.to_account_info();
        if metadata_info.data_len() == 0 {
            // CreateMetadataAccountV3 — discriminator 33
            let ix = Self::build_create_metadata_v3(
                ctx.accounts.metadata.key(),
                ctx.accounts.token_mint.key(),
                ctx.accounts.token_state.key(),
                ctx.accounts.owner.key(),
                ctx.accounts.token_state.key(),
                &data,
            );

            invoke_signed(
                &ix,
                &[
                    metadata_info,
                    ctx.accounts.token_mint.to_account_info(),
                    ctx.accounts.token_state.to_account_info(),
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.token_state.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    ctx.accounts.rent.to_account_info(),
                ],
                signer_seeds,
            )?;
        } else {
            // UpdateMetadataAccountV2 — discriminator 15
            let ix = Self::build_update_metadata_v2(
                ctx.accounts.metadata.key(),
                ctx.accounts.token_state.key(),
                &data,
            );

            invoke_signed(
                &ix,
                &[
                    metadata_info,
                    ctx.accounts.token_state.to_account_info(),
                ],
                signer_seeds,
            )?;
        }

        emit!(events::MetadataSet {
            mint: ctx.accounts.token_mint.key(),
            name: params.name.clone(),
            symbol: params.symbol.clone(),
            uri: params.uri.clone(),
        });

        Ok(())
    }

    /// Build CreateMetadataAccountV3 instruction.
    /// Instruction discriminator: 33 (CreateMetadataAccountV3)
    /// Layout: [33, data, is_mutable, collection_details]
    fn build_create_metadata_v3(
        metadata: Pubkey,
        mint: Pubkey,
        mint_authority: Pubkey,
        payer: Pubkey,
        update_authority: Pubkey,
        data: &DataV2,
    ) -> Instruction {
        // Serialize instruction data: discriminator(33) + DataV2 + is_mutable(true) + collection_details(None)
        let mut ix_data = vec![33u8];
        ix_data.extend(data.try_to_vec().unwrap());
        ix_data.push(1); // is_mutable = true
        ix_data.extend([0u8]); // collection_details = None

        Instruction {
            program_id: TOKEN_METADATA_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(metadata, false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(mint_authority, true),
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(update_authority, false),
                AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
                AccountMeta::new_readonly(anchor_lang::solana_program::sysvar::rent::ID, false),
            ],
            data: ix_data,
        }
    }

    /// Build UpdateMetadataAccountV2 instruction.
    /// Instruction discriminator: 15 (UpdateMetadataAccountV2)
    /// Layout: [15, Option<DataV2>, Option<Pubkey>, Option<bool>]
    fn build_update_metadata_v2(
        metadata: Pubkey,
        update_authority: Pubkey,
        data: &DataV2,
    ) -> Instruction {
        // Serialize: discriminator(15) + Some(data) + Some(update_authority) + Some(is_mutable=true)
        let mut ix_data = vec![15u8];
        // Option<DataV2> = Some
        ix_data.push(1);
        ix_data.extend(data.try_to_vec().unwrap());
        // Option<Pubkey> new_update_authority = Some(update_authority)
        ix_data.push(1);
        ix_data.extend(update_authority.to_bytes());
        // Option<bool> is_mutable = Some(true)  (primary_sale_happened)
        ix_data.push(0); // primary_sale_happened = None
        // Option<bool> is_mutable = Some(true)
        ix_data.push(1);
        ix_data.push(1); // true

        Instruction {
            program_id: TOKEN_METADATA_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(metadata, false),
                AccountMeta::new_readonly(update_authority, true),
            ],
            data: ix_data,
        }
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct SetMetadataParams {
    /// Token name (e.g., "LP Token").
    pub name: String,
    /// Token symbol (e.g., "LP").
    pub symbol: String,
    /// URI pointing to off-chain metadata JSON.
    pub uri: String,
}
