// In this example, we use two chips and connect them together
// The two chips are identical and simply constrain an advice cell to be the sum of two advice cells on the same row
// The chip looks like this (ignoring the selector which is always enabled when this chip is used)
// |-----|------|-------|--------------|
// | row | left | right |     out      |
// |-----|------|-------|--------------|
// |  0  |   a  |   b   |    a + b     |
// |-----|------|-------|--------------|
//
// We feed two private inputs into the first instanciation of the chip. Then, we feed the output of the first chip into the left input of the second chip, and feed another private input to the right input
//
// The full circuit looks like this
// |-----|-------|-------|--------------|-------|
// | row | left  | right |     out      |  sel  |
// |-----|-------|-------|--------------|-------|
// |  0  |   a   |   b   |    out_1     |   1   |
// |-----|-------|-------|--------------|-------|
// |  1  | out_1 |   c   |    out_2     |   1   |
// |-----|-------|-------|--------------|-------|

use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner},
    dev::MockProver,
    pasta::Fp,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Selector},
    poly::Rotation,
};

#[derive(Debug)]
struct AdditionChip<F> {
    config: AdditionChipConfig,
    marker: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct AdditionChipConfig {
    left_col: Column<Advice>,
    right_col: Column<Advice>,
    out_col: Column<Advice>,
    sel: Selector,
}

impl<F: FieldExt> Chip<F> for AdditionChip<F> {
    type Config = AdditionChipConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

impl<F: FieldExt> AdditionChip<F> {
    fn new(config: <Self as Chip<F>>::Config) -> Self {
        AdditionChip {
            config,
            marker: PhantomData,
        }
    }

    // Creates the columns and gates (constraint polynomials) required by this chip and stores
    // references to the columns in the chip config structure.
    fn configure(cs: &mut ConstraintSystem<F>) -> <Self as Chip<F>>::Config {
        // create three advice columns
        let left_col = cs.advice_column();
        let right_col = cs.advice_column();
        let out_col = cs.advice_column();

        // enable equality constraints for the left and out one.
        // Enabling this for the right one does not seem to be required, because in our usage of this chip,
        // we never use a copy constraint on the right column, only between the left and out ones
        cs.enable_equality(left_col);
        cs.enable_equality(out_col);

        // create a selector to activate this chip
        let sel = cs.selector();

        // create a gate which constrains `left + right == out` iff the selector is on
        cs.create_gate("add", |meta| {
            let left = meta.query_advice(left_col, Rotation::cur());
            let right = meta.query_advice(right_col, Rotation::cur());
            let out = meta.query_advice(out_col, Rotation::cur());
            let sel = meta.query_selector(sel);

            vec![sel * (left + right - out)]
        });

        AdditionChipConfig {
            left_col,
            right_col,
            out_col,
            sel,
        }
    }

    /// Allocate values for the first addition, based on some concrete values
    fn alloc_from_values(
        &self,
        layouter: &mut impl Layouter<F>,
        left: Option<F>,
        right: Option<F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "add two values",
            |mut region| {
                // enable this constraint
                // if this is ommited, the test will pass but the system will be underconstrained!
                self.config.sel.enable(&mut region, 0)?;

                // compute the value of the output. Just an addition, but looks more complicated because we operate on options
                let out = left.and_then(|l| right.and_then(|r| Some(r + l)));

                // we have a single row in this chip
                let row_offset = 0;

                // assign the three columns and return the assigned cell for the out column for usage later
                region.assign_advice(
                    || "left",
                    self.config.left_col,
                    row_offset,
                    || left.ok_or(Error::Synthesis),
                )?;
                region.assign_advice(
                    || "right",
                    self.config.right_col,
                    row_offset,
                    || right.ok_or(Error::Synthesis),
                )?;
                region.assign_advice(
                    || "out",
                    self.config.out_col,
                    row_offset,
                    || out.ok_or(Error::Synthesis),
                )
            },
        )
    }

    /// Allocate values for the second addition, based on the output of the first addition as well as a concrete value
    fn alloc_from_output_and_value(
        &self,
        layouter: &mut impl Layouter<F>,
        left: AssignedCell<F, F>,
        right: Option<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "add an output and a value",
            |mut region| {
                // enable this chip
                self.config.sel.enable(&mut region, 0)?;

                // compute the output based on the value of the assigned cell and the concrete value
                let out = left.value().and_then(|l| right.and_then(|r| Some(r + l)));

                let row_offset = 0;

                // add a copy constraint linking the passed cell (from anywhere in the circuit) to the left cell of this gate
                left.copy_advice(|| "left", &mut region, self.config.left_col, row_offset)?;

                // assign the right gate and return nothing as we do not do any further processing
                // we could also return the assigned out and ignore it in the caller
                region.assign_advice(
                    || "right",
                    self.config.right_col,
                    row_offset,
                    || right.ok_or(Error::Synthesis),
                )?;
                region.assign_advice(
                    || "out",
                    self.config.out_col,
                    row_offset,
                    || out.ok_or(Error::Synthesis),
                )?;

                Ok(())
            },
        )
    }
}

#[derive(Clone, Default)]
struct TwoChipCircuit<F> {
    a: Option<F>,
    b: Option<F>,
    c: Option<F>,
}

impl<F: FieldExt> Circuit<F> for TwoChipCircuit<F> {
    // Our circuit uses one chip, thus we can reuse the chip's config as the circuit's config.
    type Config = AdditionChipConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(cs: &mut ConstraintSystem<F>) -> Self::Config {
        AdditionChip::configure(cs)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // create a first chip
        let first_addition = AdditionChip::new(config.clone());

        // assign the first addition based on the first two values
        let first_addition_output =
            first_addition.alloc_from_values(&mut layouter, self.a, self.b)?;

        // create a second addition
        // TODO: is it possible to create the chips with some parameters in order to avoid having both
        // `alloc_from_values` and `alloc_from_output_and_value` which are quite similar?
        let second_addition = AdditionChip::new(config.clone());

        // assign the second addition based on the output of the first addition, and the third witness value
        second_addition.alloc_from_output_and_value(
            &mut layouter,
            first_addition_output,
            self.c,
        )?;
        Ok(())
    }
}

fn main() {
    let k = 3;

    // The prover creates a circuit containing the public and private inputs.
    let circuit = TwoChipCircuit {
        a: Some(Fp::from(1)),
        b: Some(Fp::from(2)),
        c: Some(Fp::from(3)),
    };

    // Create the area you want to draw on.
    // Use SVGBackend if you want to render to .svg instead.
    use plotters::prelude::*;
    let root = BitMapBackend::new("two_chips.png", (1024, 768)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let root = root
        .titled("Simple example with two chips", ("sans-serif", 60))
        .unwrap();

    halo2_proofs::dev::CircuitLayout::default()
        .render::<Fp, _, _>(k, &circuit, &root)
        .unwrap();

    // Assert that the constraint system is satisfied.
    let prover = MockProver::run(k, &circuit, vec![]).unwrap();
    assert!(prover.verify().is_ok());
}
