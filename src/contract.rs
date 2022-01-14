use cosmwasm_std::{entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, Order, Addr, Decimal, Coin, BankMsg, coins};

use cw2::set_contract_version;
use crate::error::ContractError;
use crate::msg::{UserAmountResponse, ExecuteMsg, InstantiateMsg, PoolTotalAmountResponse, QueryMsg, UserGainResponse};
use crate::state::{AMOUNT_BY_USER, GAIN_BY_USER, STATE, State};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cosmon-minter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = msg.admin
        .and_then(|s| deps.api.addr_validate(s.as_str()).ok())
        .unwrap_or(info.sender);

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let initial_state = State {
        admin,
        pool_total_amount: Default::default()
    };
    STATE.save(deps.storage, &initial_state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {} => stake(deps, info),
        ExecuteMsg::UpdatePoolTotalAmount {} => execute_update_pool_total(deps, info),
        ExecuteMsg::Withdraw {amount} => execute_withdraw(deps, info, amount),
    }
}

pub fn execute_withdraw(deps: DepsMut, info: MessageInfo, amount: Uint128) -> Result<Response, ContractError> {

    let gain_for_user = GAIN_BY_USER
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    let amount_for_user = AMOUNT_BY_USER
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    if amount > amount_for_user || amount == Uint128::zero() {
       return Err(ContractError::WrongArgument {name: "amount".to_string()})
    }

    let percentage_to_withdraw = Decimal::from_ratio(amount,amount_for_user);
    let gain_to_withdraw = percentage_to_withdraw * gain_for_user;

    GAIN_BY_USER.update(deps.storage, &info.sender, |gain_before_withdraw: Option<Uint128>| -> StdResult<_> {
        Ok(gain_before_withdraw.unwrap_or_default().checked_sub(gain_to_withdraw)?)
    })?;

    AMOUNT_BY_USER.update(deps.storage, &info.sender, |amount_before_withdraw: Option<Uint128>| -> StdResult<_> {
        Ok(amount_before_withdraw.unwrap_or_default().checked_sub(amount)?)
    })?;

    Ok(send_tokens(info.sender, coins((gain_to_withdraw + amount).u128(), "ubay"), "execute_withdraw"))
}

pub fn execute_update_pool_total(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {

    let state = STATE.load(deps.storage)?;
    if info.sender != state.admin {
            return Err(ContractError::Unauthorized {});
    }

    let amount_send= verify_token_deposit(info.clone())?;

    let all_user: StdResult<Vec<_>> = AMOUNT_BY_USER
        .range_raw(deps.storage, None, None, Order::Ascending).collect();
    let all_user = all_user.unwrap();
    let mut current_amount;
    let mut current_user_address;
    for user_amount in all_user {
        println!("user {} amount {}", std::str::from_utf8(&user_amount.0).unwrap(), user_amount.1);
        current_amount = user_amount.1;
        current_user_address = Addr::unchecked(std::str::from_utf8(&user_amount.0).unwrap().to_string());
        let share = Decimal::from_ratio(current_amount,state.pool_total_amount);
        let gain_by_user= share * amount_send;

        GAIN_BY_USER.update(deps.storage, &current_user_address, |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_add(gain_by_user)?)
        })?;
    }

    STATE.update(deps.storage, |mut state|-> Result<_, ContractError> {
        state.pool_total_amount += amount_send;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("method", "execute_update_pool_total"))
}

pub fn stake(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {

   let amount_send= verify_token_deposit(info.clone())?;

    AMOUNT_BY_USER.update(deps.storage, &info.sender, |balance: Option<Uint128>| -> StdResult<_> {
        Ok(balance.unwrap_or_default().checked_add(amount_send)?)
    })?;

    STATE.update(deps.storage, |mut state|-> Result<_, ContractError> {
        state.pool_total_amount += amount_send;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("method", "stake"))
}

// this is a helper to move the tokens, so the business logic is easy to read
fn send_tokens(to_address: Addr, amount: Vec<Coin>, action: &str) -> Response {
    Response::new()
        .add_message(BankMsg::Send {
            to_address: to_address.clone().into(),
            amount,
        })
        .add_attribute("action", action)
        .add_attribute("to", to_address)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetUserAmount {user} => {
            let amount = AMOUNT_BY_USER
                .may_load(deps.storage, &user)?
                .unwrap_or_default();

            to_binary(&UserAmountResponse {amount})
        },
        QueryMsg::GetPoolTotalAmount { } => {
             let state = STATE.load(deps.storage)?;
            to_binary(&PoolTotalAmountResponse{amount: state.pool_total_amount })
        }
        QueryMsg::GetUserGain { user } => {
            let amount = GAIN_BY_USER
                .may_load(deps.storage, &user)?
                .unwrap_or_default();

            to_binary(&UserGainResponse {amount})
        }
    }
}

fn verify_token_deposit(info: MessageInfo) -> Result<Uint128, ContractError> {
    let mut amount_of_coin_stackable= Uint128::zero();
    for coin in &info.funds {
        if coin.denom == "ubay" {
            amount_of_coin_stackable = coin.amount;
        }
    }
    if amount_of_coin_stackable.is_zero() {
        return Err(ContractError::WrongDeposit {})
    };

    Ok(amount_of_coin_stackable)

}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{Addr, Coin, coins, CosmosMsg, from_binary};

    fn deposit_tokens(deps: DepsMut, address: &str) {
        let info = mock_info(address, &coins(2_000_000, "ubay"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps, mock_env(), info, msg).unwrap();
    }

    fn call_update_pool_total_amount(deps: DepsMut) {
        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let msg = ExecuteMsg::UpdatePoolTotalAmount {};
        let _res = execute(deps, mock_env(), info, msg).unwrap();
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { admin: Some("creator".to_string())};
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

    }

    #[test]
    fn deposit() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { admin: Some("creator".to_string())};
        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &coins(2_000_000, "ubay"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserAmount { user: Addr::unchecked("anyone") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        assert_eq!(2_000_000u128, value.amount.u128());

        let info = mock_info("anyone", &coins(2_000_000, "ubay"));
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserAmount { user: Addr::unchecked("anyone") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        assert_eq!(4_000_000u128, value.amount.u128());

        let mut multi_coins = coins(2_000_000, "toto");
        multi_coins.push(Coin::new(1_000_000, "ubay"));
        let info = mock_info("anyone", &multi_coins);
        let msg = ExecuteMsg::Deposit {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserAmount { user: Addr::unchecked("anyone") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        assert_eq!(5_000_000u128, value.amount.u128());

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetPoolTotalAmount {  }).unwrap();

        let value: PoolTotalAmountResponse = from_binary(&res).unwrap();
        assert_eq!(5_000_000u128, value.amount.u128());

        let info = mock_info("anyone", &coins(2_000_000, "toto"));
        let msg = ExecuteMsg::Deposit {};
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        match res {
                    Err(ContractError::WrongDeposit {}) => {}
                    _ => panic!("Must return WrongDeposit error"),
        }
    }

    #[test]
    fn call_update_pool_total_amount_1_user() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { admin: Some("creator".to_string())};
        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // 1 user in the pool
        deposit_tokens(deps.as_mut(), "user1");

        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let msg = ExecuteMsg::UpdatePoolTotalAmount {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserGain { user: Addr::unchecked("user1") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        assert_eq!(2_000_000u128, value.amount.u128());

    }
    #[test]
    fn call_update_pool_total_amount_3_users() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { admin: Some("creator".to_string())};
        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        deposit_tokens(deps.as_mut(), "user1");
        deposit_tokens(deps.as_mut(), "user2");
        deposit_tokens(deps.as_mut(), "user3");

        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let msg = ExecuteMsg::UpdatePoolTotalAmount {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserGain { user: Addr::unchecked("user3") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        assert_eq!(666_666u128, value.amount.u128());

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetPoolTotalAmount {  }).unwrap();

        let value: PoolTotalAmountResponse = from_binary(&res).unwrap();
        assert_eq!(8_000_000u128, value.amount.u128());

        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let msg = ExecuteMsg::UpdatePoolTotalAmount {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserGain { user: Addr::unchecked("user3") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        // 666_666 + (25% of 2_000_000) = 1_166_666
        assert_eq!(1_166_666u128, value.amount.u128());

    }

    #[test]
    fn withdraw_all_amount() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { admin: Some("creator".to_string())};
        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        deposit_tokens(deps.as_mut(), "user1");
        call_update_pool_total_amount(deps.as_mut());

        let info = mock_info("user1", &[]);
        let msg = ExecuteMsg::Withdraw { amount: Uint128::zero() };
        let _res = execute(deps.as_mut(), mock_env(), info, msg);
        match _res {
            Err(ContractError::WrongArgument { name }) => { assert_eq!(name, "amount".to_string()) },
            _ => panic!("Must return WrongDeposit error"),
        }

        let info = mock_info("user1", &[]);
        let msg = ExecuteMsg::Withdraw { amount: Uint128::from(2_000_001u128) };
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        match res {
            Err(ContractError::WrongArgument { name }) => { assert_eq!(name, "amount".to_string()) },
            _ => panic!("Must return WrongDeposit error"),
        }

        let info = mock_info("user1", &[]);
        let msg = ExecuteMsg::Withdraw { amount: Uint128::from(2_000_000u128) };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(1, res.messages.len());
        let msg = res.messages.get(0).unwrap();
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "user1".into(),
                amount: coins(4_000_000, "ubay"),
            }));

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserGain { user: Addr::unchecked("user1") }).unwrap();

        let value: UserGainResponse = from_binary(&res).unwrap();
        assert_eq!(value.amount, Uint128::zero());

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserAmount { user: Addr::unchecked("user1") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        assert_eq!(value.amount, Uint128::zero());

    }

    #[test]
    fn withdraw_half_amount() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { admin: Some("creator".to_string())};
        let info = mock_info("creator", &coins(2_000_000, "ubay"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        deposit_tokens(deps.as_mut(), "user1");
        call_update_pool_total_amount(deps.as_mut());

        let info = mock_info("user1", &[]);
        let msg = ExecuteMsg::Withdraw { amount: Uint128::from(1_000_000u128) };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        assert_eq!(1, res.messages.len());
        let msg = res.messages.get(0).unwrap();
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "user1".into(),
                amount: coins(2_000_000, "ubay"),
            }));

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserGain { user: Addr::unchecked("user1") }).unwrap();

        let value: UserGainResponse = from_binary(&res).unwrap();
        assert_eq!(value.amount, Uint128::from(1_000_000u128));

        let res = query(deps.as_ref(),
                        mock_env(),
                        QueryMsg::GetUserAmount { user: Addr::unchecked("user1") }).unwrap();

        let value: UserAmountResponse = from_binary(&res).unwrap();
        assert_eq!(value.amount, Uint128::from(1_000_000u128));

    }

}
