# coswasm-staking

Contract deploying on [sandynet](https://sandynet.aneka.io/accounts/wasm1l340jm4hw6ltay3cwyu4pg97p532qr5lmchqwv)
## General Info
The contract has 3 *execute* functions:
- `deposit` to deposit funds
- `withdraw` with an `amount` in parameter to withdraw this amount plus the gain
- `update_pool_total_amount` the admin call it to fill the contract with reward

And you can query internal state using `get_user_amount`, `get_user_gain`, `get_pool_total_amount`.

##Interact
In order to interact with it:
- Follow the [tutorial](https://docs.cosmwasm.com/tutorials/simple-option/setup) to setup your environment
- For an "execute message" type in bash:
  - execute name and parameter: eg `QUERY='{"withdraw": {"amount":"100"}}'`
  - execute the "transaction" `wasmd tx wasm execute $CONTRACT "$QUERY" --from wallet $TXFLAG -y`
- For a query type in bash:
  - query name and parameter: eg `NAME_QUERY='{"get_user_amount": {"user": "user@"}}'`
  - execute the "query" `wasmd query wasm contract-state smart $CONTRACT "$NAME_QUERY" $NODE --output json`
