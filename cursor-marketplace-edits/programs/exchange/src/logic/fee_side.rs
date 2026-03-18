use crate::state::types::*;

/// Equivalent to LibFeeSide.getFeeSide.
/// Determines which side of the trade pays the protocol fee.
/// Priority: Sol -> WrappedSol -> SplToken -> SemiFungible -> None
pub fn get_fee_side(
    maker_asset_class: &AssetClass,
    taker_asset_class: &AssetClass,
) -> FeeSide {
    if *maker_asset_class == AssetClass::Sol {
        return FeeSide::Take;
    }
    if *taker_asset_class == AssetClass::Sol {
        return FeeSide::Make;
    }
    if *maker_asset_class == AssetClass::WrappedSol {
        return FeeSide::Take;
    }
    if *taker_asset_class == AssetClass::WrappedSol {
        return FeeSide::Make;
    }
    if *maker_asset_class == AssetClass::SplToken {
        return FeeSide::Take;
    }
    if *taker_asset_class == AssetClass::SplToken {
        return FeeSide::Make;
    }
    if *maker_asset_class == AssetClass::SemiFungible {
        return FeeSide::Take;
    }
    if *taker_asset_class == AssetClass::SemiFungible {
        return FeeSide::Make;
    }
    FeeSide::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sol_priority() {
        assert_eq!(get_fee_side(&AssetClass::Sol, &AssetClass::Nft), FeeSide::Take);
        assert_eq!(get_fee_side(&AssetClass::Nft, &AssetClass::Sol), FeeSide::Make);
    }

    #[test]
    fn test_wsol_priority() {
        assert_eq!(get_fee_side(&AssetClass::WrappedSol, &AssetClass::Nft), FeeSide::Take);
        assert_eq!(get_fee_side(&AssetClass::Nft, &AssetClass::WrappedSol), FeeSide::Make);
    }

    #[test]
    fn test_spl_priority() {
        assert_eq!(get_fee_side(&AssetClass::SplToken, &AssetClass::Nft), FeeSide::Take);
        assert_eq!(get_fee_side(&AssetClass::Nft, &AssetClass::SplToken), FeeSide::Make);
    }

    #[test]
    fn test_none() {
        assert_eq!(get_fee_side(&AssetClass::Nft, &AssetClass::Nft), FeeSide::None);
    }
}
