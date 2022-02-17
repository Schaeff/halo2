// In this example, we have three variables: two instances and one advice
// To showcase how to get instances in and out of the circuit, we constrain:
// - the first instance to the advice
// - the advice to the second instance

// Imperative pseudocode for this program would be:
// def main(public field a) -> field:
//    field b = a // assign private to public
//    return b    // return public

const FIRST_PUB_INPUT_ROW_INDEX: usize = 0;
const SECOND_PUB_INPUT_ROW_INDEX: usize = 1;

use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner},
    dev::MockProver,
    pasta::Fp,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance},
};

#[derive(Debug)]
struct MyChip<F> {
    config: MyChipConfig,
    marker: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct MyChipConfig {
    priv_col: Column<Advice>,
    pub_col: Column<Instance>,
}

impl<F: FieldExt> Chip<F> for MyChip<F> {
    type Config = MyChipConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

impl<F: FieldExt> MyChip<F> {
    fn new(config: <Self as Chip<F>>::Config) -> Self {
        MyChip {
            config,
            marker: PhantomData,
        }
    }

    // Creates the columns and gates (constraint polynomials) required by this chip and stores
    // references to the columns in the chip config structure.
    fn configure(cs: &mut ConstraintSystem<F>) -> <Self as Chip<F>>::Config {
        let priv_col = cs.advice_column();
        cs.enable_equality(priv_col);

        let pub_col = cs.instance_column();
        cs.enable_equality(pub_col);

        MyChipConfig { priv_col, pub_col }
    }

    // Constrain and assign a private cell to the first public input
    fn alloc_first_input(
        &self,
        layouter: &mut impl Layouter<F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "expose first input",
            |mut region| {
                let row_offset = 0;
                region.assign_advice_from_instance(
                    || "public input advice",
                    self.config.pub_col,
                    FIRST_PUB_INPUT_ROW_INDEX,
                    self.config.priv_col,
                    row_offset,
                )
            },
        )
    }

    // Constrain and assign the second public input to the private cell
    fn alloc_second_input(
        &self,
        layouter: &mut impl Layouter<F>,
        cell: AssignedCell<F, F>,
    ) -> Result<(), Error> {
        layouter.constrain_instance(cell.cell(), self.config.pub_col, SECOND_PUB_INPUT_ROW_INDEX)
    }
}

#[derive(Clone, Default)]
struct MyCircuit;

impl<F: FieldExt> Circuit<F> for MyCircuit {
    // Our circuit uses one chip, thus we can reuse the chip's config as the circuit's config.
    type Config = MyChipConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(cs: &mut ConstraintSystem<F>) -> Self::Config {
        MyChip::configure(cs)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let chip = MyChip::new(config);
        let assigned_private_cell = chip.alloc_first_input(&mut layouter)?;
        chip.alloc_second_input(&mut layouter, assigned_private_cell)?;
        Ok(())
    }
}

fn main() {
    // The circuit's public input for both instances
    const PUB_INPUT: u64 = 5;

    let k = 3;

    // we repeat the same value for both instances
    let pub_inputs = vec![Fp::from(PUB_INPUT), Fp::from(PUB_INPUT)];

    // The prover creates a circuit containing the public and private inputs.
    let circuit = MyCircuit;

    // Assert that the constraint system is satisfied.
    let prover = MockProver::run(k, &circuit, vec![pub_inputs.clone()]).unwrap();
    assert!(prover.verify().is_ok());

    // Assert that changing the public inputs results in the constraint system becoming unsatisfied.
    let mut bad_pub_inputs = pub_inputs.clone();
    bad_pub_inputs[FIRST_PUB_INPUT_ROW_INDEX] = Fp::from(PUB_INPUT + 1);
    let prover = MockProver::run(k, &circuit, vec![bad_pub_inputs]).unwrap();
    assert!(prover.verify().is_err());
}
