//! Manual whole-source writer review bound to literal source anchors and layout.
//! This is not an automatic call-graph proof or a production allowlist.
use super::*;
fn classification(t: Target, label: &str) -> Result<(&'static str, &'static str, Vec<&'static str>)> {
    let value=match(t,label){
        (Target::Bnbtiger,"_owner")=>("runtime","Ownable constructor; waiveOwnership, transferOwnership and lock are onlyOwner; unlock requires previous owner and elapsed lock time.",vec!["_owner = msgSender;","_owner = address(0);","_owner = newOwner;","_owner = _previousOwner;"]),
        (Target::Bnbtiger,"_previousOwner")=>("runtime","onlyOwner lock; retained for unlock.",vec!["_previousOwner = _owner;"]),
        (Target::Bnbtiger,"_lockTime")=>("runtime","onlyOwner lock; timestamp-dependent setter is outside execution scope.",vec!["_lockTime = block.timestamp + time;"]),
        (Target::Bnbtiger,"_name"|"_symbol"|"_decimals")=>("constructor_only","Declaration initializers only; no runtime setter. Name and symbol are short literals. Synthetic string corruption below does not admit dynamic-string writes.",vec!["string private _name = \"Bnb Tiger Inu\";","string private _symbol = \"BNBTiger\";","uint8 private _decimals = 9;"]),
        (Target::Bnbtiger,"marketingWalletAddress")=>("runtime","Declaration initializer and onlyOwner setMarketingWalletAddress; shares slot 5 with decimals.",vec!["marketingWalletAddress = payable(newAddress);"]),
        (Target::Bnbtiger,"teamWalletAddress")=>("runtime","Declaration initializer and onlyOwner setTeamWalletAddress.",vec!["teamWalletAddress = payable(newAddress);"]),
        (Target::Bnbtiger,"_balances")=>("balance_input","Constructor allocation and transfer/transferFrom through _transfer, _basicTransfer and takeFee. Transfer/router paths are not executed here.",vec!["function _basicTransfer(","function takeFee(","return _balances[account];"]),
        (Target::Bnbtiger,"_allowances")=>("runtime","Constructor router allowance; approve/increaseAllowance/decreaseAllowance/transferFrom and swap approval through _approve.",vec!["function _approve(","function increaseAllowance(","function decreaseAllowance(","function transferFrom("]),
        (Target::Bnbtiger,"isExcludedFromFee")=>("runtime","Constructor and onlyOwner setIsExcludedFromFee.",vec!["isExcludedFromFee[account] = newValue;"]),
        (Target::Bnbtiger,"isWalletLimitExempt")=>("runtime","Constructor, onlyOwner setIsWalletLimitExempt and changeRouterVersion.",vec!["isWalletLimitExempt[holder] = exempt;","function changeRouterVersion("]),
        (Target::Bnbtiger,"isTxLimitExempt")=>("runtime","Constructor and onlyOwner setIsTxLimitExempt.",vec!["isTxLimitExempt[holder] = exempt;"]),
        (Target::Bnbtiger,"isMarketPair")=>("runtime","Constructor, onlyOwner setMarketPairStatus and changeRouterVersion.",vec!["isMarketPair[account] = newValue;","function changeRouterVersion("]),
        (Target::Bnbtiger,"_buyLiquidityFee"|"_buyMarketingFee"|"_buyTeamFee"|"_totalTaxIfBuying")=>("runtime","Declaration/constructor values and onlyOwner setBuyTaxes; total recomputed with checked additions.",vec!["_buyLiquidityFee = newLiquidityTax;","_buyMarketingFee = newMarketingTax;","_buyTeamFee = newTeamTax;","_totalTaxIfBuying = _buyLiquidityFee.add(_buyMarketingFee).add(_buyTeamFee);"]),
        (Target::Bnbtiger,"_sellLiquidityFee"|"_sellMarketingFee"|"_sellTeamFee"|"_totalTaxIfSelling")=>("runtime","Declaration/constructor values and onlyOwner setSellTaxes; total recomputed with checked additions.",vec!["_sellLiquidityFee = newLiquidityTax;","_sellMarketingFee = newMarketingTax;","_sellTeamFee = newTeamTax;","_totalTaxIfSelling = _sellLiquidityFee.add(_sellMarketingFee).add(_sellTeamFee);"]),
        (Target::Bnbtiger,"_liquidityShare"|"_marketingShare"|"_teamShare"|"_totalDistributionShares")=>("runtime","Declaration/constructor values and onlyOwner setDistributionSettings; total recomputed.",vec!["_liquidityShare = newLiquidityShare;","_marketingShare = newMarketingShare;","_teamShare = newTeamShare;","_totalDistributionShares = _liquidityShare.add(_marketingShare).add(_teamShare);"]),
        (Target::Bnbtiger,"_totalSupply")=>("constructor_only","Declaration initializer; no mint/burn or runtime supply assignment in the complete selected contract. Direct balances need no supply conversion.",vec!["uint256 private _totalSupply = 10000000000000 * 10**6* 10**6 * 10**_decimals;"]),
        (Target::Bnbtiger,"_maxTxAmount")=>("runtime","Declaration and onlyOwner setMaxTxAmount.",vec!["_maxTxAmount = maxTxAmount;"]),
        (Target::Bnbtiger,"_walletMax")=>("runtime","Declaration and onlyOwner setWalletLimit.",vec!["_walletMax  = newLimit;"]),
        (Target::Bnbtiger,"minimumTokensBeforeSwap")=>("runtime","Declaration and onlyOwner setNumTokensBeforeSwap.",vec!["minimumTokensBeforeSwap = newLimit;"]),
        (Target::Bnbtiger,"uniswapV2Router"|"uniswapPair")=>("runtime_external_context","Constructor and onlyOwner changeRouterVersion use external router/factory calls. These are source-reviewed, not executed or qualified.",vec!["function changeRouterVersion(","uniswapPair = newPairAddress;","uniswapV2Router = _uniswapV2Router;"]),
        (Target::Bnbtiger,"inSwapAndLiquify")=>("runtime","lockTheSwap modifier enters/exits during swapAndLiquify; external transfer path excluded.",vec!["inSwapAndLiquify = true;","inSwapAndLiquify = false;"]),
        (Target::Bnbtiger,"swapAndLiquifyEnabled")=>("runtime","Declaration and onlyOwner setSwapAndLiquifyEnabled.",vec!["swapAndLiquifyEnabled = _enabled;"]),
        (Target::Bnbtiger,"swapAndLiquifyByLimitOnly")=>("runtime","Declaration and onlyOwner setSwapAndLiquifyByLimitOnly.",vec!["swapAndLiquifyByLimitOnly = newValue;"]),
        (Target::Bnbtiger,"checkWalletLimit")=>("runtime","Declaration and onlyOwner enableDisableWalletLimit.",vec!["checkWalletLimit = newValue;"]),
        (Target::Cookie,"_owner")=>("runtime","Ownable constructor, onlyOwner renounceOwnership and transferOwnership.",vec!["function renounceOwnership(","function transferOwnership("]),
        (Target::Cookie,"_balances")=>("balance_input","BEP20 _transfer and _mint reachable from transfers and two onlyOwner mint overloads. _burn/_burnFrom are declared internal but have no public caller in this complete selected source; sending to BURN_ADDRESS is a balance transfer.",vec!["function _transfer(","function _mint(","function mint(uint256 amount)","function mint(address _to, uint256 _amount)","return _balances[account];"]),
        (Target::Cookie,"_allowances")=>("runtime","_approve reached by approve/increaseAllowance/decreaseAllowance/transferFrom and swap helpers. _burnFrom has no public call path.",vec!["function _approve(","function approve(","function transferFrom("]),
        (Target::Cookie,"_totalSupply")=>("runtime","_mint is reached by both onlyOwner mint overloads. _burn is an unreachable internal definition here; transfer to BURN_ADDRESS does not reduce supply.",vec!["_totalSupply = _totalSupply.add(amount);","function mint(uint256 amount)","function mint(address _to, uint256 _amount)"]),
        (Target::Cookie,"_name"|"_symbol"|"_decimals")=>("constructor_only","BEP20 constructor assignments only; Cookie Token/COOKIE are short literals. No runtime name/symbol/decimal setter. Dynamic-string host corruption is an independence control only.",vec!["_name = name;","_symbol = symbol;","_decimals = 18;"]),
        (Target::Cookie,"transferTaxRate")=>("runtime","Declaration, onlyOperator updateTransferTaxRate bounded <=1000, and transferTaxFree temporary zero/restore.",vec!["transferTaxRate = _transferTaxRate;","transferTaxRate = 0;","_transferTaxRate <= MAXIMUM_TRANSFER_TAX_RATE"]),
        (Target::Cookie,"burnRate")=>("runtime","Declaration and onlyOperator updateBurnRate bounded <=100.",vec!["burnRate = _burnRate;","_burnRate <= 100"]),
        (Target::Cookie,"maxTransferAmountRate")=>("runtime","Declaration and onlyOperator updateMaxTransferAmountRate bounded <=500.",vec!["maxTransferAmountRate = _maxTransferAmountRate;","_maxTransferAmountRate <= 500"]),
        (Target::Cookie,"_excludedFromAntiWhale")=>("runtime","Constructor and onlyOperator setExcludedFromAntiWhale.",vec!["_excludedFromAntiWhale[_account] = _excluded;"]),
        (Target::Cookie,"swapAndLiquifyEnabled")=>("runtime","Declaration and onlyOperator updateSwapAndLiquifyEnabled.",vec!["swapAndLiquifyEnabled = _enabled;"]),
        (Target::Cookie,"minAmountToLiquify")=>("runtime","Declaration and onlyOperator updateMinAmountToLiquify.",vec!["minAmountToLiquify = _minAmount;"]),
        (Target::Cookie,"cookieSwapRouter"|"cookieSwapPair")=>("runtime_external_context","onlyOperator updateCookieSwapRouter calls router/factory and requires a nonzero pair. No successful external execution is claimed.",vec!["cookieSwapRouter = IUniswapV2Router02(_router);","cookieSwapPair = IUniswapV2Factory(cookieSwapRouter.factory()).getPair(address(this), cookieSwapRouter.WETH());"]),
        (Target::Cookie,"_inSwapAndLiquify")=>("runtime","lockTheSwap modifier around excluded swap/router path.",vec!["_inSwapAndLiquify = true;","_inSwapAndLiquify = false;"]),
        (Target::Cookie,"maxHoldingRate")=>("constructor_only","Declaration value 500 only; no setter in complete selected source. It affects blacklist/transfer policy, not raw balanceOf.",vec!["uint16 public maxHoldingRate = 500;"]),
        (Target::Cookie,"_includeToBlackList")=>("runtime","Constructor false entries; onlyOperator setExcludeFromBlackList and conditional setIncludeToBlackList. The latter reads balanceOf/maxHolding.",vec!["_includeToBlackList[_account] = false;","_includeToBlackList[_account] = true;"]),
        (Target::Cookie,"_operator")=>("runtime","Constructor and onlyOperator transferOperator, nonzero new operator.",vec!["_operator = _msgSender();","_operator = newOperator;"]),
        (Target::Cookie,"_delegates")=>("runtime","_delegate from delegate or delegateBySig; signature/context path not executed.",vec!["_delegates[delegator] = delegatee;","function delegate(address delegatee)","function delegateBySig("]),
        (Target::Cookie,"checkpoints")=>("runtime_context","_writeCheckpoint through _moveDelegates from mint(address,uint256), delegate and delegateBySig. Same-block changes votes only; new record writes uint32 block plus uint256 votes. Block/signature context not executed.",vec!["checkpoints[delegatee][nCheckpoints - 1].votes = newVotes;","checkpoints[delegatee][nCheckpoints] = Checkpoint(blockNumber, newVotes);"]),
        (Target::Cookie,"numCheckpoints")=>("runtime_context","_writeCheckpoint appends a new uint32-indexed record. safe32 checks the block conversion; nCheckpoints + 1 wraps as unchecked uint32 arithmetic in Solidity 0.6.12. Neither path is executed here.",vec!["numCheckpoints[delegatee] = nCheckpoints + 1;","safe32(block.number,"]),
        (Target::Cookie,"nonces")=>("runtime_context","delegateBySig postincrement after signature recovery; expiry/time/chain/signature path excluded.",vec!["nonce == nonces[signatory]++"]),
        _=>anyhow::bail!("unreviewed declaration {label}"),
    };
    Ok(value)
}
pub fn review(c: &Value, t: Target) -> Result<Value> {
    let expected: Vec<(&str, u64, u64)> = if t == Target::Bnbtiger {
        vec![
            ("_owner", 0, 0),
            ("_previousOwner", 1, 0),
            ("_lockTime", 2, 0),
            ("_name", 3, 0),
            ("_symbol", 4, 0),
            ("_decimals", 5, 0),
            ("marketingWalletAddress", 5, 1),
            ("teamWalletAddress", 6, 0),
            ("_balances", 7, 0),
            ("_allowances", 8, 0),
            ("isExcludedFromFee", 9, 0),
            ("isWalletLimitExempt", 10, 0),
            ("isTxLimitExempt", 11, 0),
            ("isMarketPair", 12, 0),
            ("_buyLiquidityFee", 13, 0),
            ("_buyMarketingFee", 14, 0),
            ("_buyTeamFee", 15, 0),
            ("_sellLiquidityFee", 16, 0),
            ("_sellMarketingFee", 17, 0),
            ("_sellTeamFee", 18, 0),
            ("_liquidityShare", 19, 0),
            ("_marketingShare", 20, 0),
            ("_teamShare", 21, 0),
            ("_totalTaxIfBuying", 22, 0),
            ("_totalTaxIfSelling", 23, 0),
            ("_totalDistributionShares", 24, 0),
            ("_totalSupply", 25, 0),
            ("_maxTxAmount", 26, 0),
            ("_walletMax", 27, 0),
            ("minimumTokensBeforeSwap", 28, 0),
            ("uniswapV2Router", 29, 0),
            ("uniswapPair", 30, 0),
            ("inSwapAndLiquify", 30, 20),
            ("swapAndLiquifyEnabled", 30, 21),
            ("swapAndLiquifyByLimitOnly", 30, 22),
            ("checkWalletLimit", 30, 23),
        ]
    } else {
        vec![
            ("_owner", 0, 0),
            ("_balances", 1, 0),
            ("_allowances", 2, 0),
            ("_totalSupply", 3, 0),
            ("_name", 4, 0),
            ("_symbol", 5, 0),
            ("_decimals", 6, 0),
            ("transferTaxRate", 6, 1),
            ("burnRate", 6, 3),
            ("maxTransferAmountRate", 6, 5),
            ("_excludedFromAntiWhale", 7, 0),
            ("swapAndLiquifyEnabled", 8, 0),
            ("minAmountToLiquify", 9, 0),
            ("cookieSwapRouter", 10, 0),
            ("cookieSwapPair", 11, 0),
            ("_inSwapAndLiquify", 11, 20),
            ("maxHoldingRate", 11, 21),
            ("_includeToBlackList", 12, 0),
            ("_operator", 13, 0),
            ("_delegates", 14, 0),
            ("checkpoints", 15, 0),
            ("numCheckpoints", 16, 0),
            ("nonces", 17, 0),
        ]
    };
    let fields = c["storageLayout"]["storage"].as_array().context("layout")?;
    ensure!(fields.len() == expected.len(), "all declarations reviewed");
    let mut records = vec![];
    for (f, (label, slot, offset)) in fields.iter().zip(expected) {
        let expected_slot = slot.to_string();
        ensure!(
            f["label"] == label && f["slot"].as_str() == Some(expected_slot.as_str()) && f["offset"] == offset,
            "reviewed exact declaration location {label}"
        );
        let (classification, reason, anchors) = classification(t, label)?;
        let mut evidence = vec![];
        for anchor in anchors {
            let mut found = false;
            for (path, s) in c["sources"].as_object().context("sources")? {
                let s = s["content"].as_str().context("body")?;
                for (start, _) in s.match_indices(anchor) {
                    found = true;
                    evidence.push(json!({"path":path,"source_sha256":sha(s.as_bytes()),"byte_start":start,"byte_length":anchor.len(),"line":s[..start].bytes().filter(|b|*b==b'\n').count()+1,"literal":anchor}));
                }
            }
            ensure!(found, "source anchor {label}: {anchor}");
        }
        records.push(json!({"declaration":f,"classification":classification,"review":reason,"anchors":evidence}));
    }
    Ok(
        json!({"target":t.label(),"method":"Manual complete selected-source review with literal source anchors; not an automatic reachability proof or metadata permission","fields":records,"dynamic_string_limit":"No long-string write admission; string head/data mutations test only getter independence","executed_scope":"balanceOf only","immutable_dead_address":if t==Target::Bnbtiger{json!("0x000000000000000000000000000000000000dead")}else{Value::Null}}),
    )
}
