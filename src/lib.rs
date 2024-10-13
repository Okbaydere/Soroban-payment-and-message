#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec, Map, symbol_short};
use soroban_sdk::testutils::arbitrary::std::println;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

#[contract]
pub struct PaymentMessagingSystem;

#[contracttype]
#[derive(Clone, Debug)]
pub struct Payment {
    from: Address,
    to: Address,
    amount: i128,
    message: String,
}

#[contracttype]
#[derive(Clone)]
pub struct RecurringPayment {
    to: Address,
    amount: i128,
    interval: u64,
    message: String,
    last_payment: u64,
}

#[contractimpl]
impl PaymentMessagingSystem {
    // Balance query
    pub fn balance(env: Env, token_id: Address, address: Address) -> i128 {
        address.require_auth();
        let token = TokenClient::new(&env, &token_id);
        let balance = token.balance(&address);
        println!("Balance query: Address: {:?}, Token ID: {:?}, Balance: {:?}", address, token_id, balance); // Debug print
        balance
    }

    // XLM transfer and message sending
    pub fn transfer(env: Env, token_id: Address, from: Address, to: Address, amount: i128, message: String) -> bool {
        from.require_auth();
        let token = TokenClient::new(&env, &token_id);

        println!("Initiating transfer: From: {:?}, To: {:?}, Amount: {:?}, Message: {:?}", from, to, amount, message); // Debug print

        token.transfer(&from, &to, &amount);

        // Store payment record
        let mut payments = Self::get_payments(&env, &from);
        payments.push_back(Payment {
            from: from.clone(),
            to: to.clone(),
            amount,
            message: message.clone(),
        });
        Self::set_payments(&env, &from, &payments);

        println!("Transfer successful: From: {:?}, To: {:?}, Amount: {:?}", from, to, amount); // Debug print
        true
    }

    // Create payment plan for recurring payments
    pub fn create_recurring_payment(env: Env, from: Address, to: Address, amount: i128, interval: u64, message: String) {
        from.require_auth();
        let mut recurring_payments = Self::get_recurring_payments(&env);
        recurring_payments.set(from.clone(), RecurringPayment {
            to: to.clone(),
            amount,
            interval,
            message: message.clone(),
            last_payment: env.ledger().timestamp(),
        });
        Self::set_recurring_payments(&env, &recurring_payments);
        println!("Recurring payment created: From: {:?}, To: {:?}, Amount: {:?}, Interval: {:?}, Message: {:?}", from, to, amount, interval, message); // Debug print
    }

    // Multi-recipient transfer
    pub fn multi_transfer(env: Env, token_id: Address, from: Address, recipients: Vec<(Address, i128)>, message: String) -> bool {
        from.require_auth();
        let token = TokenClient::new(&env, &token_id);

        println!("Initiating multi-transfer: From: {:?}, Recipients: {:?}, Message: {:?}", from, recipients, message); // Debug print

        for (to, amount) in recipients.iter() {
            token.transfer(&from, &to, &amount);

            // Store payment record
            let mut payments = Self::get_payments(&env, &from);
            payments.push_back(Payment {
                from: from.clone(),
                to: to.clone(),
                amount: amount, // Dereference the amount
                message: message.clone(),
            });
            Self::set_payments(&env, &from, &payments);
            println!("Transferred: From: {:?}, To: {:?}, Amount: {:?}", from, to, amount); // Debug print
        }

        println!("Multi-transfer successful: From: {:?}", from); // Debug print
        true
    }

    // View transaction history
    pub fn get_transaction_history(env: Env, address: Address) -> Vec<Payment> {
        address.require_auth();
        let history = Self::get_payments(&env, &address);
        println!("Transaction history for: {:?}, History: {:?}", address, history); // Debug print
        history
    }

    // Helper functions
    fn get_payments(env: &Env, address: &Address) -> Vec<Payment> {
        let key = (symbol_short!("payments"), address.clone());
        env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env))
    }

    fn set_payments(env: &Env, address: &Address, payments: &Vec<Payment>) {
        let key = (symbol_short!("payments"), address.clone());
        env.storage().persistent().set(&key, payments);
    }

    fn get_recurring_payments(env: &Env) -> Map<Address, RecurringPayment> {
        env.storage().persistent().get(&symbol_short!("recurring")).unwrap_or_else(|| Map::new(env))
    }

    fn set_recurring_payments(env: &Env, recurring_payments: &Map<Address, RecurringPayment>) {
        env.storage().persistent().set(&symbol_short!("recurring"), recurring_payments);
    }

    // Process recurring payments
    pub fn process_recurring_payments(env: Env, token_id: Address) {
        let current_timestamp = env.ledger().timestamp();
        let mut recurring_payments = Self::get_recurring_payments(&env);
        let token = TokenClient::new(&env, &token_id);

        println!("Processing recurring payments at timestamp: {:?}", current_timestamp); // Debug print

        for (from, mut payment) in recurring_payments.iter() {
            if current_timestamp >= payment.last_payment + payment.interval {
                // Perform the payment
                from.require_auth();
                token.transfer(&from, &payment.to, &payment.amount);

                // Update last payment time
                payment.last_payment = current_timestamp;
                recurring_payments.set(from.clone(), payment.clone());

                // Store payment record
                let mut payments = Self::get_payments(&env, &from);
                payments.push_back(Payment {
                    from: from.clone(),
                    to: payment.to.clone(),
                    amount: payment.amount,
                    message: payment.message.clone(),
                });
                Self::set_payments(&env, &from, &payments);

                println!("Processed recurring payment: From: {:?}, To: {:?}, Amount: {:?}", from, payment.to, payment.amount); // Debug print
            }
        }

        Self::set_recurring_payments(&env, &recurring_payments);
    }
}

#[cfg(test)]
mod test {
    use soroban_sdk::vec;
    use super::*;
    use soroban_sdk::testutils::{Address as TestAddress, Ledger, LedgerInfo};

    const INITIAL_MINT_AMOUNT: i128 = 1_000_000_000;

    use soroban_sdk::{Env, Address, String as SorobanString};

    fn create_token_contract(env: &Env) -> Address {
        let contract_id_str = String::from_str(env, "GA5DLODYBEZBKY3GCSVU42N6YARV4LCYGWIZVI5SSKFIAJTKYMFXB5DI");
        let contract_address = Address::from_string(&contract_id_str);
        env.register_stellar_asset_contract_v2(contract_address.clone());
        let client = StellarAssetClient::new(env, &contract_address); // fixed to pass Address type
        let recipient = <soroban_sdk::Address as TestAddress>::generate(env);
        client.mint(&recipient, &INITIAL_MINT_AMOUNT);
        println!("Token contract created: {:?}", contract_address); // Debug print
        contract_address
    }

    fn setup_test_env<'a>() -> (Env, PaymentMessagingSystemClient<'a>, Address) {
        let env = Env::default();
        let contract_id = env.register_contract(None, PaymentMessagingSystem);
        let client = PaymentMessagingSystemClient::new(&env, &contract_id);
        let token_id = create_token_contract(&env);
        (env, client, token_id)
    }

    #[test]
    fn test_transfer() {
        let (env, client, token_id) = setup_test_env();
        let sender = <soroban_sdk::Address as TestAddress>::generate(&env);
        let recipient = <soroban_sdk::Address as TestAddress>::generate(&env);

        env.mock_all_auths();
        let result = client.transfer(&token_id, &sender, &recipient, &10i128, &String::from_str(&env, "Test payment"));
        assert!(result);

        env.mock_all_auths();
        let balance = client.balance(&token_id, &recipient);
        println!("Recipient balance after transfer: {:?}", balance); // Debug print
        assert_eq!(balance, 10i128);
    }

    #[test]
    fn test_recurring_payment() {
        let (env, client, token_id) = setup_test_env();
        let sender = <soroban_sdk::Address as TestAddress>::generate(&env);
        let recipient = <soroban_sdk::Address as TestAddress>::generate(&env);

        env.mock_all_auths();
        client.create_recurring_payment(&sender, &recipient, &10i128, &86400u64, &String::from_str(&env, "Daily payment"));
        println!("Recurring payment created from {:?} to {:?}", sender, recipient); // Debug print

        env.ledger().set(LedgerInfo {
            timestamp: 100000,
            protocol_version: 20,
            sequence_number: 123,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 10,
            min_persistent_entry_ttl: 10,
            max_entry_ttl: 3110400,
        });

        client.process_recurring_payments(&token_id);

        env.mock_all_auths();
        let history = client.get_transaction_history(&sender);
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().amount, 10i128);
        assert_eq!(history.get(0).unwrap().message, String::from_str(&env, "Daily payment"));
        println!("Transaction history for sender: {:?}", history); // Debug print
    }

    #[test]
    fn test_multi_transfer() {
        let (env, client, token_id) = setup_test_env();
        let sender = <soroban_sdk::Address as TestAddress>::generate(&env);
        let user2 = <soroban_sdk::Address as TestAddress>::generate(&env);
        let user3 = <soroban_sdk::Address as TestAddress>::generate(&env);

        // Construct the recipients vector
        let recipients = vec![
            &env,
            (user2.clone(), 10i128),
            (user3.clone(), 20i128),
        ];

        env.mock_all_auths();
        let result = client.multi_transfer(&token_id, &sender, &recipients, &String::from_str(&env, "Multi transfer"));
        assert!(result);

        env.mock_all_auths();
        let history = client.get_transaction_history(&sender);
        assert_eq!(history.len(), 2);
        assert_eq!(history.get(0).unwrap().amount, 10i128);
        assert_eq!(history.get(1).unwrap().amount, 20i128);
        println!("Transaction history for sender after multi-transfer: {:?}", history); // Debug print
    }
}
