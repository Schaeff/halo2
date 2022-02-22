/// Prove that private `x` is in the range [3, 7]
///
/// We use a lookup of `x` in a table containing [3, 7]
use std::marker::PhantomData;

use halo2_proofs::arithmetic::FieldExt;
use halo2_proofs::circuit::{Chip, Layouter, SimpleFloorPlanner};
use halo2_proofs::plonk::{Advice, Circuit, Column, ConstraintSystem, Error, TableColumn};
use halo2_proofs::poly::Rotation;

use halo2_proofs::plonk::{Expression, Selector};

#[derive(Debug, Clone)]
pub struct RangeCheckChipConfig {
    x: Column<Advice>,
    selector: Selector,
    range_table: TableColumn,
}

#[derive(Clone)]
pub struct RangeCheckChip<F> {
    config: RangeCheckChipConfig,
    marker: PhantomData<F>,
}

impl<F: FieldExt> Chip<F> for RangeCheckChip<F> {
    type Config = RangeCheckChipConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

impl<F: FieldExt> RangeCheckChip<F> {
    fn new(config: <Self as Chip<F>>::Config) -> Self {
        RangeCheckChip {
            config,
            marker: PhantomData,
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> <Self as Chip<F>>::Config {
        let x = meta.advice_column();
        let selector = meta.complex_selector();
        let range_table = meta.lookup_table_column();

        meta.lookup(|meta| {
            let x = meta.query_advice(x, Rotation::cur());
            let sel = meta.query_selector(selector);
            vec![(
                sel.clone() * x + (Expression::Constant(F::one()) - sel) * F::from(3),
                range_table,
            )]
        });

        RangeCheckChipConfig {
            x,
            range_table,
            selector,
        }
    }

    fn assign_private(&self, layouter: &mut impl Layouter<F>, x: Option<F>) -> Result<(), Error> {
        layouter.assign_region(
            || "assign x",
            |mut region| {
                let offset = 0;
                self.config.selector.enable(&mut region, 0)?;
                region.assign_advice(|| "x", self.config.x, offset, || x.ok_or(Error::Synthesis))
            },
        )?;
        Ok(())
    }

    fn assign_table(&self, layouter: &mut impl Layouter<F>) -> Result<(), Error> {
        layouter.assign_table(
            || format!("range [{}, {}]", 3, 7),
            |mut table| {
                for (i, v) in (3..8).enumerate() {
                    table.assign_cell(
                        || format!("{}", v),
                        self.config.range_table,
                        i,
                        || Ok(F::from(v)),
                    )?;
                }
                Ok(())
            },
        )?;
        Ok(())
    }
}

#[derive(Default)]
struct RangeCheckCircuit<F> {
    x: Option<F>,
}

impl<F: FieldExt> Circuit<F> for RangeCheckCircuit<F> {
    type Config = RangeCheckChipConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        RangeCheckChip::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let chip = RangeCheckChip::<F>::new(config);
        chip.assign_private(&mut layouter, self.x)?;
        chip.assign_table(&mut layouter)?;
        Ok(())
    }
}

fn main() {
    use halo2_proofs::{dev::MockProver, pasta::Fp};

    let k = 4;

    // create the private input
    let x = Fp::from(4);

    // create the circuit using the private inputs
    let circuit = RangeCheckCircuit { x: Some(x) };

    use plotters::prelude::*;
    let root = BitMapBackend::new("lookup_range_check.png", (1024, 768)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let root = root
        .titled("Range check example Layout", ("sans-serif", 60))
        .unwrap();

    halo2_proofs::dev::CircuitLayout::default()
        .render(k, &circuit, &root)
        .unwrap();

    // run the prover!
    let verify = MockProver::run(k, &circuit, vec![]).unwrap().verify();
    assert!(verify.is_ok());

    // change the witness and check that it fails
    let bad_circuit = RangeCheckCircuit {
        x: Some(Fp::from(42)),
    };
    let verify = MockProver::run(k, &bad_circuit, vec![]).unwrap().verify();
    assert!(verify.is_err());

    // change the witness to zero and check it fails (the default value should not be accepted)
    let bad_circuit = RangeCheckCircuit {
        x: Some(Fp::zero()),
    };
    let verify = MockProver::run(k, &bad_circuit, vec![]).unwrap().verify();
    assert!(verify.is_err());
}
