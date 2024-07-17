use scrypto::prelude::*;

// This module defines a Call Money contract blueprint.
// Call Money is a financial instrument where the lender can demand repayment at any time.
#[blueprint]
mod call_money {
    /// The CallMoney struct represents the state of a Call Money contract.
    struct CallMoney {
        // Parties involved in the contract
        lender: ResourceAddress,           // Address of the lender's account
        borrower: ResourceAddress,         // Address of the borrower's account

        // Financial details
        principal: Decimal,                // The original amount borrowed
        interest_rate: Decimal,            // Annual interest rate (as a decimal, e.g., 0.05 for 5%)
        accrued_interest: Decimal,         // Interest accumulated but not yet paid

        // Time-related fields
        start_date: i64,                   // Unix timestamp of when the contract started
        last_interest_calculation_date: i64, // Last date interest was calculated
        notice_period: i64,                // Required notice period (in seconds) before repayment
        grace_period: i64,                 // Grace period (in seconds) after due date before penalties apply

        // Contract state
        status: String,                    // Current status of the contract (e.g., "Active", "Called", "Repaid")

        // Additional features
        penalty_rate: Decimal,             // Rate at which penalties accrue if repayment is late
        collateral: Option<ResourceAddress>, // Optional collateral provided by the borrower

        // Record keeping
        transaction_history: Vec<String>,  // Log of all transactions and status changes
    }

    impl CallMoney {
        /// Instantiates a new Call Money contract.
        /// 
        /// # Arguments
        /// * `lender` - ResourceAddress of the lender
        /// * `borrower` - ResourceAddress of the borrower
        /// * `principal` - The amount being borrowed
        /// * `interest_rate` - Annual interest rate (as a decimal)
        /// * `start_date` - Unix timestamp of the contract start date
        /// * `notice_period` - Required notice period in seconds
        /// * `grace_period` - Grace period in seconds
        /// * `penalty_rate` - Rate at which penalties accrue if repayment is late
        ///
        /// # Returns
        /// A tuple containing the ComponentAddress of the new contract and an owner_badge Bucket
        pub fn instantiate_call_money(
            lender: ResourceAddress,
            borrower: ResourceAddress,
            principal: Decimal,
            interest_rate: Decimal,
            start_date: i64,
            notice_period: i64,
            grace_period: i64,
            penalty_rate: Decimal,
        ) -> Global<CallMoney> {
            // Input validation
            assert!(principal > Decimal::ZERO, "Principal must be positive");
            assert!(interest_rate > Decimal::ZERO && interest_rate < Decimal::ONE, "Interest rate must be between 0 and 1");
            assert!(notice_period >= 0, "Notice period cannot be negative");
            assert!(grace_period >= 0, "Grace period cannot be negative");
            assert!(penalty_rate >= Decimal::ZERO, "Penalty rate cannot be negative");

            // Create the CallMoney instance
            Self {
                lender,
                borrower,
                principal,
                interest_rate,
                start_date,
                accrued_interest: Decimal::ZERO,
                last_interest_calculation_date: start_date,
                status: "Active".to_string(),
                notice_period,
                grace_period,
                penalty_rate,
                collateral: None,
                transaction_history: vec!["Contract initiated".to_string()],
            }.instantiate()
            .prepare_to_globalize(OwnerRole::None)
            .globalize()

            // Instantiate the component, create an owner badge, and globalize
            // let (address, owner_badge) = Self::instantiate(call_money)
            //     .with_owner_badge()
            //     .globalize();

            // Return the component address and owner badge
            // (address, owner_badge)
        }

        /// Updates the accrued interest based on the time passed since the last calculation.
        ///
        /// # Arguments
        /// * `current_date` - The current date as a Unix timestamp
        pub fn update_accrued_interest(&mut self, current_date: i64) {
            // Calculate the number of days since the last interest calculation
            let days = (current_date - self.last_interest_calculation_date) as i128;
            
            // Calculate the interest accrued over this period
            let interest = self.principal * self.interest_rate * Decimal::from(days) / Decimal::from(365);
            
            // Add the calculated interest to the accrued interest
            self.accrued_interest += interest;
            
            // Update the last interest calculation date
            self.last_interest_calculation_date = current_date;
            
            // Log this transaction
            self.transaction_history.push(format!("Interest updated: {}", interest));
        }

        /// Processes a repayment on the loan.
        ///
        /// # Arguments
        /// * `amount` - The amount being repaid
        /// * `current_date` - The current date as a Unix timestamp
        ///
        /// # Returns
        /// Any excess payment that exceeds the total amount due
        pub fn repay(&mut self, amount: Decimal, current_date: i64) -> Decimal {
            // Update the accrued interest before processing the repayment
            self.update_accrued_interest(current_date);
            
            // Calculate the total amount due
            let total_due = self.principal + self.accrued_interest;
            
            if amount >= total_due {
                // If the payment covers or exceeds the total due
                self.status = "Repaid".to_string();
                let excess = amount - total_due;
                self.principal = Decimal::ZERO;
                self.accrued_interest = Decimal::ZERO;
                self.transaction_history.push(format!("Loan fully repaid. Excess: {}", excess));
                excess // Return any excess payment
            } else {
                // If it's a partial payment
                self.accrued_interest -= amount;
                if self.accrued_interest < Decimal::ZERO {
                    // If the payment exceeds the accrued interest, apply the remainder to the principal
                    self.principal += self.accrued_interest;
                    self.accrued_interest = Decimal::ZERO;
                }
                self.transaction_history.push(format!("Partial repayment: {}", amount));
                Decimal::ZERO // No excess payment
            }
        }

        /// Initiates the process of calling the money back.
        ///
        /// # Arguments
        /// * `current_date` - The current date as a Unix timestamp
        ///
        /// # Returns
        /// A tuple containing the total amount due and the due date
        pub fn call_money(&mut self, current_date: i64) -> (Decimal, i64) {
            assert!(self.status == "Active", "Contract is not active");
            
            // Update the accrued interest
            self.update_accrued_interest(current_date);
            
            // Calculate the total amount due
            let total_due = self.principal + self.accrued_interest;
            
            // Mark the contract as called
            self.status = "Called".to_string();
            
            // Calculate the due date
            let due_date = current_date + self.notice_period;
            
            // Log this action
            self.transaction_history.push(format!("Money called. Due on: {}", due_date));
            
            (total_due, due_date)
        }

        /// Applies a penalty if the repayment is overdue.
        ///
        /// # Arguments
        /// * `current_date` - The current date as a Unix timestamp
        pub fn apply_penalty(&mut self, current_date: i64) {
            assert!(self.status == "Called", "Contract has not been called");
            
            // Get the due date from the call_money method
            let (_, due_date) = self.call_money(current_date);
            
            // Check if we're past the grace period
            if current_date > due_date + self.grace_period {
                // Calculate the number of days overdue
                let days_overdue = (current_date - (due_date + self.grace_period)) as i128;
                
                // Calculate the penalty
                let penalty = self.principal * self.penalty_rate * Decimal::from(days_overdue) / Decimal::from(365);
                
                // Add the penalty to the accrued interest
                self.accrued_interest += penalty;
                
                // Log this action
                self.transaction_history.push(format!("Penalty applied: {}", penalty));
            }
        }

        /// Adds collateral to the contract.
        ///
        /// # Arguments
        /// * `collateral` - The ResourceAddress of the collateral being added
        pub fn add_collateral(&mut self, collateral: ResourceAddress) {
            assert!(self.collateral.is_none(), "Collateral already exists");
            self.collateral = Some(collateral);
            self.transaction_history.push("Collateral added".to_string());
        }

        /// Removes and returns the collateral, if the loan is fully repaid.
        ///
        /// # Returns
        /// The ResourceAddress of the collateral, if it exists and the loan is repaid
        pub fn remove_collateral(&mut self) -> Option<ResourceAddress> {
            assert!(self.principal == Decimal::ZERO, "Loan must be fully repaid to remove collateral");
            let collateral = self.collateral.take();
            if collateral.is_some() {
                self.transaction_history.push("Collateral removed".to_string());
            }
            collateral
        }

        /// Retrieves the current details of the contract.
        ///
        /// # Returns
        /// A tuple containing all the current contract details
        pub fn get_details(&self) -> (ResourceAddress, ResourceAddress, Decimal, Decimal, i64, Decimal, String, Option<ResourceAddress>) {
            (
                self.lender,
                self.borrower,
                self.principal,
                self.interest_rate,
                self.start_date,
                self.accrued_interest,
                self.status.clone(),
                self.collateral,
            )
        }

        /// Retrieves the full transaction history of the contract.
        ///
        /// # Returns
        /// A vector of strings, each representing a transaction or status change
        pub fn get_transaction_history(&self) -> Vec<String> {
            self.transaction_history.clone()
        }
    }
}