/// Prove that private x is in the range [3, 7]
///
use std::marker::PhantomData;

use halo2_proofs::arithmetic::FieldExt;
use halo2_proofs::circuit::{Chip, Layouter, SimpleFloorPlanner};
use halo2_proofs::plonk::{Advice, Circuit, Column, ConstraintSystem, Error};
use halo2_proofs::poly::Rotation;

use halo2_proofs::plonk::{Expression, Selector};

/// The config for our addition circuit. It stores the two advices and the instance
/// A selector was added because of the "cell poisoned error"
#[derive(Debug, Clone)]
pub struct RangeCheckChipConfig {
    x: Column<Advice>,
    s: Selector,
}

// The addition circuit with the two private inputs which we feed during witness generation
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
        let s = meta.selector();

        // we create the gate, which constrains the cells. However, we do not specify witness generation here
        // we will do that when synthesizing
        meta.create_gate("range_check", |meta| {
            let x = meta.query_advice(x, Rotation::cur());
            let s = meta.query_selector(s);
            vec![
                s * (3..8)
                    .map(|i| (x.clone() - Expression::Constant(F::from(i))))
                    .fold(Expression::Constant(F::from(1)), |acc, e| e * acc),
            ]
        });

        RangeCheckChipConfig { x, s }
    }

    fn assign_private(&self, layouter: &mut impl Layouter<F>, x: Option<F>) -> Result<(), Error> {
        layouter.assign_region(
            || "assign x",
            |mut region| {
                self.config.s.enable(&mut region, 0)?;
                region.assign_advice(|| "x", self.config.x, 0, || x.ok_or(Error::Synthesis))
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

    // plot the circuit in `layout.png`
    use plotters::prelude::*;
    let root = BitMapBackend::new("range_check.png", (1024, 768)).into_drawing_area();
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
}
