// This example creates a single-chip circuit that proves knowledge of two field elements `a` and
// `b` such that `a^2 + b^2 == PI^2` (for public input `PI`), i.e. proves knowledge of a Pythagorean
// triple.
//
// The arithmetization for this computation is:
//
// (0) allocate private inputs: a, b
// (1) allocate public input: c = PI
// (2) multiply: a * a = a^2
// (3) multiply: b * b = b^2
// (4) multiply: c * c = c^2
// (5) add: a^2 + b^2 = c^2
//
// The constraint system has 3 advice columns `l` (left), `r` (right), and `o` (output), one
// instance column `pub_col` (contains the public inputs), and 3 selectors (fixed columns) `s_add`
// (addition gate), `s_mul` (multiplication gate), and `s_pub` (public input gate).
//
// |-----|-------|-------|-------|---------|-------|-------|-------|
// | row | l_col | r_col | o_col | pub_col | s_add | s_mul | s_pub |
// |-----|-------|-------|-------|---------|-------|-------|-------|
// |  0  |   a   |   b   |       |   0     |   0   |   0   |   0   |
// |  1  |   c   |       |       |   PI    |   0   |   0   |   1   |
// |  2  |   a   |   a   |  aa   |   0     |   0   |   1   |   0   |
// |  3  |   b   |   b   |  bb   |   0     |   0   |   1   |   0   |
// |  4  |   c   |   c   |  cc   |   0     |   0   |   1   |   0   |
// |  5  |   aa  |   bb  |  cc   |   0     |   1   |   0   |   0   |
// |-----|-------|-------|-------|---------|-------|-------|-------|
//
// Any advice value that appears in multiple rows has the consistency of its value enforced across
// rows via permutation argument, e.g. row #0 `a` == row #2 `a` is enforced within in the
// permutation argument.

// The row index of the public input. This is the only absolute index we use,
// everything else is relative offsets within a region
const PUB_INPUT_ROW_INDEX: usize = 1;

use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner},
    dev::MockProver,
    pasta::Fp,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance, Selector},
    poly::Rotation,
};

#[derive(Debug)]
struct MyChip<F> {
    config: MyChipConfig,
    marker: PhantomData<F>,
}

#[derive(Clone, Debug)]
struct MyChipConfig {
    l_col: Column<Advice>,
    r_col: Column<Advice>,
    o_col: Column<Advice>,
    pub_col: Column<Instance>,
    s_add: Selector,
    s_mul: Selector,
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
        let l_col = cs.advice_column();
        cs.enable_equality(l_col);
        let r_col = cs.advice_column();
        cs.enable_equality(r_col);
        let o_col = cs.advice_column();
        cs.enable_equality(o_col);

        // We won't store a reference to the public input column in the config structure because the
        // column's values will be provided by the verifier, i.e. the chip will never assign values
        // into `pub_col`; the selector is used only to defining gates.
        let pub_col = cs.instance_column();
        cs.enable_equality(pub_col);

        let s_add = cs.selector();
        let s_mul = cs.selector();

        // Define the addition gate.
        //
        // | l_col | r_col | o_col | s_add |
        // |-------|-------|-------|-------|
        // |   l   |   r   |   o   | s_add |
        //
        // Constraint: s_add*l + s_add*r = s_add*o
        cs.create_gate("add", |cs| {
            let l = cs.query_advice(l_col, Rotation::cur());
            let r = cs.query_advice(r_col, Rotation::cur());
            let o = cs.query_advice(o_col, Rotation::cur());
            let s_add = cs.query_selector(s_add);
            vec![s_add * (l + r - o)]
        });

        // Define the multiplication gate.
        //
        // | l_col | r_col | o_col | s_mul |
        // |-------|-------|-------|-------|
        // |   l   |   r   |   o   | s_mul |
        //
        // Constraint: s_mul*l*r = s_mul*o
        cs.create_gate("mul", |cs| {
            let l = cs.query_advice(l_col, Rotation::cur());
            let r = cs.query_advice(r_col, Rotation::cur());
            let o = cs.query_advice(o_col, Rotation::cur());
            let s_mul = cs.query_selector(s_mul);
            vec![s_mul * (l * r - o)]
        });

        MyChipConfig {
            l_col,
            r_col,
            o_col,
            pub_col,
            s_add,
            s_mul,
        }
    }

    // In the next available row, writes `a` into the row's left cell and `b` into the row's right
    // cell.
    fn alloc_private_inputs(
        &self,
        layouter: &mut impl Layouter<F>,
        a: Option<F>,
        b: Option<F>,
    ) -> Result<(AssignedCell<F, F>, AssignedCell<F, F>), Error> {
        layouter.assign_region(
            || "load private inputs",
            |mut region| {
                let row_offset = 0;
                let a_cell = region.assign_advice(
                    || "private input 'a'",
                    self.config.l_col,
                    row_offset,
                    || a.ok_or(Error::Synthesis),
                )?;
                let b_cell = region.assign_advice(
                    || "private input 'b'",
                    self.config.r_col,
                    row_offset,
                    || b.ok_or(Error::Synthesis),
                )?;
                // Note that no arithmetic is performed here, all we are doing is allocating the
                // initial private wire values (i.e. private values which are not the output of any
                // gate), thus there is no selector enabled in this row.
                Ok((a_cell, b_cell))
            },
        )
    }

    // Set the left column of the next available row to the value of the instance
    // This is not only witness generation: under the hood, it constrains the two cell to be equal
    fn alloc_public_input(
        &self,
        layouter: &mut impl Layouter<F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "expose public input",
            |mut region| {
                let row_offset = 0;
                // No selector is being used here
                region.assign_advice_from_instance(
                    || "public input advice",
                    self.config.pub_col,
                    PUB_INPUT_ROW_INDEX,
                    self.config.l_col,
                    row_offset,
                )
            },
        )
    }

    // In the next available row, copies a previously allocated value `prev_alloc` into the row's left
    // and right cells, then writes the product of the left and right cells into the row's output
    // cell; enabling `s_mul` in the row enforces that the left, right, and output cells satisfy the
    // multiplication constraint: `l * r = o`.
    fn square(
        &self,
        layouter: &mut impl Layouter<F>,
        prev_alloc: AssignedCell<F, F>,
    ) -> Result<AssignedCell<F, F>, Error> {
        let squared_value = prev_alloc.value().map(|x| *x * x);
        layouter.assign_region(
            || "square",
            |mut region| {
                let row_offset = 0;
                self.config.s_mul.enable(&mut region, row_offset)?;

                let _ = prev_alloc.copy_advice(|| "l", &mut region, self.config.l_col, row_offset);
                let _ = prev_alloc.copy_advice(|| "r", &mut region, self.config.r_col, row_offset);

                region.assign_advice(
                    || "l * r",
                    self.config.o_col,
                    row_offset,
                    || squared_value.ok_or(Error::Synthesis),
                )
            },
        )
    }

    // In the next available row, copies the previously allocated values `l_prev_alloc`, `r_prev_alloc`,
    // and `o_prev_alloc` into the row's left, right, and output cells respectively. Enabling the
    // `s_add` selector enforces that the values written in the row satisfy the addition constraint
    // `l + r = o`.
    //
    // This function is called `constrained_add` because the output of `l + r` is provided by the
    // function caller as a previously allocated value.
    fn constrained_add(
        &self,
        layouter: &mut impl Layouter<F>,
        l_in_alloc: AssignedCell<F, F>,
        r_in_alloc: AssignedCell<F, F>,
        o_in_alloc: AssignedCell<F, F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "constrained add",
            |mut region| {
                let row_offset = 0;
                self.config.s_add.enable(&mut region, row_offset)?;

                let _ =
                    l_in_alloc.copy_advice(|| "l", &mut region, self.config.l_col, row_offset)?;
                let _ =
                    r_in_alloc.copy_advice(|| "r", &mut region, self.config.r_col, row_offset)?;
                let _ =
                    o_in_alloc.copy_advice(|| "o", &mut region, self.config.o_col, row_offset)?;

                Ok(())
            },
        )
    }
}

#[derive(Clone, Default)]
struct MyCircuit<F> {
    // Private inputs.
    a: Option<F>,
    b: Option<F>,
}

impl<F: FieldExt> Circuit<F> for MyCircuit<F> {
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
        let (a_alloc, b_alloc) = chip.alloc_private_inputs(&mut layouter, self.a, self.b)?;
        let c_alloc = chip.alloc_public_input(&mut layouter)?;
        let a_sq_alloc = chip.square(&mut layouter, a_alloc)?;
        let b_sq_alloc = chip.square(&mut layouter, b_alloc)?;
        let c_sq_alloc = chip.square(&mut layouter, c_alloc)?;
        chip.constrained_add(&mut layouter, a_sq_alloc, b_sq_alloc, c_sq_alloc)
    }
}

fn main() {
    // The number of rows utilized in the constraint system matrix.
    const N_ROWS_USED: u32 = 6;

    // The circuit's public input `c` where `a^2 + b^2 = c^2` for private inputs `a` and `b`.
    const PUB_INPUT: u64 = 5;

    // The verifier creates the public inputs column (instance column). The total number of
    // rows `n_rows` in our constraint system cannot exceed 2^k, i.e.
    // `n_rows = 2^(floor(log2(N_ROWS_USED)))`.
    let k = (N_ROWS_USED as f32).log2().ceil() as u32;

    // seems like that's not enough, add one...
    let k = k + 1;

    let pub_inputs = vec![Fp::from(0), Fp::from(PUB_INPUT)];

    // The prover creates a circuit containing the public and private inputs.
    let circuit = MyCircuit {
        a: Some(Fp::from(3)),
        b: Some(Fp::from(4)),
    };

    // Assert that the constraint system is satisfied.
    let prover = MockProver::run(k, &circuit, vec![pub_inputs.clone()]).unwrap();
    assert!(prover.verify().is_ok());

    // Assert that changing the public inputs results in the constraint system becoming unsatisfied.
    let mut bad_pub_inputs = pub_inputs.clone();
    bad_pub_inputs[PUB_INPUT_ROW_INDEX] = Fp::from(PUB_INPUT + 1);
    let prover = MockProver::run(k, &circuit, vec![bad_pub_inputs]).unwrap();
    assert!(prover.verify().is_err());

    // Assert that changing a private input results in the constraint system becoming unsatisfied.
    let mut bad_circuit = circuit.clone();
    bad_circuit.b = Some(Fp::from(5));
    let prover = MockProver::run(k, &bad_circuit, vec![pub_inputs]).unwrap();
    assert!(prover.verify().is_err());
}
