/// For a public `c`, prove knowledge of `a` and `b` so that `a + b == c`
///
// The full circuit looks like this. On the first row we constrain c == PI
// |-----|-------|-------|--------------|-----------|----------|
// | row | left  | right |     out      |    pub    |  add_sel |
// |-----|-------|-------|--------------|-----------|----------|
// |  0  |   c   |       |              |    PI     |    0     |
// |-----|-------|-------|--------------|-----------|----------|
// |  1  |   a   |   b   |      c       |           |    1     |
// |-----|-------|-------|--------------|-----------|----------|
use std::marker::PhantomData;

use add::AddChip;
use halo2_proofs::arithmetic::FieldExt;
use halo2_proofs::circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner};
use halo2_proofs::plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance, Selector};
use halo2_proofs::poly::Rotation;
use public_import::PublicImportChip;

mod add {
    use super::*;

    /// The config for our addition circuit. It stores the two advices and the instance
    /// A selector was added because of the "cell poisoned error"
    #[derive(Debug, Clone)]
    pub struct AddChipConfig {
        a: Column<Advice>,
        b: Column<Advice>,
        c: Column<Advice>,
        s_add: Selector,
    }

    // The addition circuit with the two private inputs which we feed during witness generation
    #[derive(Clone)]
    pub struct AddChip<F> {
        config: AddChipConfig,
        marker: PhantomData<F>,
    }

    impl<F: FieldExt> Chip<F> for AddChip<F> {
        type Config = AddChipConfig;
        type Loaded = ();

        fn config(&self) -> &Self::Config {
            &self.config
        }

        fn loaded(&self) -> &Self::Loaded {
            &()
        }
    }

    impl<F: FieldExt> AddChip<F> {
        pub fn new(config: <Self as Chip<F>>::Config) -> Self {
            AddChip {
                config,
                marker: PhantomData,
            }
        }

        pub fn configure(meta: &mut ConstraintSystem<F>) -> <Self as Chip<F>>::Config {
            let s_add = meta.selector();
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();

            // enable the columns for equality
            meta.enable_equality(a);
            meta.enable_equality(b);
            meta.enable_equality(c);

            // we create the gate, which constrains the cells. However, we do not specify witness generation here
            // we will do that when synthesizing
            meta.create_gate("addition", |meta| {
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                let s_add = meta.query_selector(s_add);

                // if s_add is 1, a + b is constrained to c
                // if s_add is 0, any value works for a, b, c
                // this enables using these cells for something else and minimize the number of rows
                // however here it seems like we have a single row, so I'm surprised this is required
                vec![s_add * (a + b - c)]
            });

            AddChipConfig { a, b, c, s_add }
        }

        pub fn assign_sum(
            &self,
            layouter: &mut impl Layouter<F>,
            a: Option<F>,
            b: Option<F>,
        ) -> Result<AssignedCell<F, F>, Error> {
            layouter.assign_region(
                || "assign sum",
                |mut meta| {
                    self.config.s_add.enable(&mut meta, 0)?;

                    let sum = a.and_then(|a| b.and_then(|b| Some(a + b)));

                    meta.assign_advice(|| "a", self.config.a, 0, || a.ok_or(Error::Synthesis))?;
                    meta.assign_advice(|| "b", self.config.b, 0, || b.ok_or(Error::Synthesis))?;
                    meta.assign_advice(|| "sum", self.config.c, 0, || sum.ok_or(Error::Synthesis))
                },
            )
        }
    }
}

mod public_import {

    use halo2_proofs::circuit::AssignedCell;

    use super::*;

    /// The config for our addition circuit. It stores the two advices and the instance
    /// A selector was added because of the "cell poisoned error"
    #[derive(Debug, Clone)]
    pub struct PublicImportChipConfig {
        priv_col: Column<Advice>,
        pub_col: Column<Instance>,
    }

    // The addition circuit with the two private inputs which we feed during witness generation
    #[derive(Clone)]
    pub struct PublicImportChip<F> {
        config: PublicImportChipConfig,
        marker: PhantomData<F>,
    }

    impl<F: FieldExt> Chip<F> for PublicImportChip<F> {
        type Config = PublicImportChipConfig;
        type Loaded = ();

        fn config(&self) -> &Self::Config {
            &self.config
        }

        fn loaded(&self) -> &Self::Loaded {
            &()
        }
    }

    impl<F: FieldExt> PublicImportChip<F> {
        pub fn new(config: <Self as Chip<F>>::Config) -> Self {
            PublicImportChip {
                config,
                marker: PhantomData,
            }
        }

        pub fn configure(meta: &mut ConstraintSystem<F>) -> <Self as Chip<F>>::Config {
            let priv_col = meta.advice_column();
            let pub_col = meta.instance_column();

            // enable the columns for equality
            meta.enable_equality(priv_col);
            meta.enable_equality(pub_col);

            PublicImportChipConfig { priv_col, pub_col }
        }

        pub fn assign_from_public(
            &self,
            layouter: &mut impl Layouter<F>,
        ) -> Result<AssignedCell<F, F>, Error> {
            layouter.assign_region(
                || "import public",
                |mut meta| {
                    meta.assign_advice_from_instance(
                        || "copy pub to priv",
                        self.config.pub_col,
                        0,
                        self.config.priv_col,
                        0,
                    )
                },
            )
        }
    }
}

#[derive(Clone)]
struct AddPreimageCircuitConfig {
    pub_import_config: public_import::PublicImportChipConfig,
    add_config: add::AddChipConfig,
}

#[derive(Default)]
struct AddPreimageCircuit<F> {
    a: Option<F>,
    b: Option<F>,
}

impl<F: FieldExt> Circuit<F> for AddPreimageCircuit<F> {
    type Config = AddPreimageCircuitConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let pub_import_config = public_import::PublicImportChip::configure(meta);
        let add_config = add::AddChip::configure(meta);

        AddPreimageCircuitConfig {
            pub_import_config,
            add_config,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // assign the public input
        let public_import_chip = PublicImportChip::new(config.pub_import_config);
        let public_imported_cell = public_import_chip.assign_from_public(&mut layouter)?;

        // assign the sum
        let sum_chip = AddChip::new(config.add_config);
        let sum_cell = sum_chip.assign_sum(&mut layouter, self.a, self.b)?;

        // constrain the two
        layouter.assign_region(
            || "constrain",
            |mut meta| meta.constrain_equal(sum_cell.cell(), public_imported_cell.cell()),
        )?;

        Ok(())
    }
}

fn main() {
    use halo2_proofs::{dev::MockProver, pasta::Fp};

    // set the size of the circuit k. Here a single constraint should be enough so k = 2 -> 2**k = 8 (0, 1, 2 didnt work)
    let k = 3;

    // Given a public input `c`, we prove knowledge of `a` and `b` so that `a + b == c`. All variables are field elements of the prime field of the Pasta curve.

    // create public input
    let c = Fp::from(7);
    // create private inputs
    let a = Fp::from(3);
    let b = Fp::from(4);

    // create the circuit using the private inputs
    let circuit = AddPreimageCircuit {
        a: Some(a),
        b: Some(b),
    };

    // plot the circuit in `layout.png`
    use plotters::prelude::*;
    let root = BitMapBackend::new("add.png", (1024, 768)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let root = root
        .titled("Example Circuit Layout", ("sans-serif", 60))
        .unwrap();

    halo2_proofs::dev::CircuitLayout::default()
        .render(k, &circuit, &root)
        .unwrap();

    // create the public input vector or instance
    let instance = vec![vec![c]];

    // run the prover!
    let verify = MockProver::run(k, &circuit, instance.clone())
        .unwrap()
        .verify();
    assert!(verify.is_ok());

    // change the instance and check that it fails
    let bad_instance = vec![vec![Fp::from(42)]];
    let verify = MockProver::run(k, &circuit, bad_instance.clone())
        .unwrap()
        .verify();
    assert!(verify.is_err());

    // change the witness and check that it fails
    let bad_circuit = AddPreimageCircuit {
        a: Some(Fp::from(42)),
        ..circuit
    };
    let verify = MockProver::run(k, &bad_circuit, instance.clone())
        .unwrap()
        .verify();
    assert!(verify.is_err());
}
