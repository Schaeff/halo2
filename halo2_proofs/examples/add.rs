use halo2_proofs::arithmetic::Field;
use halo2_proofs::circuit::{Layouter, SimpleFloorPlanner};
use halo2_proofs::plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance, Selector};
use halo2_proofs::poly::Rotation;

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
    let circuit = Add {
        a: Some(a),
        b: Some(b),
    };

    // plot the circuit in `layout.png`
    use plotters::prelude::*;
    let root = BitMapBackend::new("layout.png", (1024, 768)).into_drawing_area();
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
    println!("{:#?}", verify);
    assert!(verify.is_ok());

    // change the instance and run again
    let bad_instance = vec![Fp::from(42)];
    let verify = MockProver::run(k, &circuit, vec![bad_instance.clone()])
        .unwrap()
        .verify();
    println!("{:#?}", verify);
    assert!(verify.is_err());
}

/// The config for our addition circuit. It stores the two advices and the instance
/// A selector was added because of the "cell poisoned error"
#[derive(Clone)]
struct AddConfig {
    a: Column<Advice>,
    b: Column<Advice>,
    c: Column<Instance>,
    s_add: Selector,
}

// The addition circuit with the two private inputs which we feed during witness generation
#[derive(Clone)]
struct Add<F> {
    a: Option<F>,
    b: Option<F>,
}

impl<F: Field> Circuit<F> for Add<F> {
    type Config = AddConfig;
    type FloorPlanner = SimpleFloorPlanner;

    // generate a witness-less circuit for steps which do not require a witness
    fn without_witnesses(&self) -> Self {
        Add {
            a: None,
            b: None,
            ..self.clone()
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        // using a selector here because the "poisoned cell" error suggested it may be required
        let s_add = meta.selector();
        let a = meta.advice_column();
        let b = meta.advice_column();
        let c = meta.instance_column();

        // we create the gate, which constrains the cells. However, we do not specify witness generation here
        // we will do that when synthesizing
        meta.create_gate("addition", |meta| {
            let v_a = meta.query_advice(a, Rotation::cur());
            let v_b = meta.query_advice(b, Rotation::cur());
            let v_c = meta.query_instance(c, Rotation::cur());
            let s_add = meta.query_selector(s_add);

            // if s_add is 1, the v_a + v_b is constrained to v_c
            // if s_add is 0, any value works for v_a, v_b, v_c
            // this enables using these cells for something else and minimize the number of rows
            // however here it seems like we have a single row, so I'm surprised this is required
            vec![s_add * (v_a + v_b - v_c)]
        });

        AddConfig { a, b, c, s_add }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // think witness generation

        // I'm not sure if these should be different regions or all the same, and whether the return value of the region assignment should be used somewhere
        let _ = layouter.assign_region(
            || "a",
            |mut region| {
                region.assign_advice(|| "a", config.a, 0, || self.a.ok_or(Error::Synthesis))
            },
        );

        let _ = layouter.assign_region(
            || "b",
            |mut region| {
                region.assign_advice(|| "b", config.b, 0, || self.b.ok_or(Error::Synthesis))
            },
        );

        let _ = layouter.assign_region(|| "add", |mut region| config.s_add.enable(&mut region, 0));

        Ok(())
    }
}
