use super::circuit::Expression;
use crate::arithmetic::{CurveAffine, FieldExt};
use ff::Field;

pub(crate) mod prover;
pub(crate) mod verifier;

#[derive(Default, Clone, Debug)]
pub struct Proof<C: CurveAffine> {
    input_commitment: C,
    table_commitment: C,
    product_commitment: C,
    pub evals: Evals<C::Scalar>,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Evals<F: FieldExt> {
    product_eval: F,
    product_next_eval: F,
    permuted_input_eval: F,
    permuted_input_inv_eval: F,
    permuted_table_eval: F,
}

#[derive(Clone, Debug)]
pub(crate) struct Argument<F: Field> {
    pub input_expressions: Vec<Expression<F>>,
    pub table_expressions: Vec<Expression<F>>,
}

impl<F: Field> Argument<F> {
    /// Constructs a new lookup argument.
    ///
    /// `table_map` is a sequence of `(input, table)` tuples.
    pub fn new(table_map: Vec<(Expression<F>, Expression<F>)>) -> Self {
        let (input_expressions, table_expressions) = table_map.into_iter().unzip();
        Argument {
            input_expressions,
            table_expressions,
        }
    }

    pub(crate) fn required_degree(&self) -> usize {
        assert_eq!(self.input_expressions.len(), self.table_expressions.len());

        // The first value in the permutation poly should be one.
        // degree 2:
        // l_0(X) * (1 - z(X)) = 0
        //
        // The "last" value in the permutation poly should be a boolean, for
        // completeness and soundness.
        // degree 3:
        // l_last(X) * (z(X)^2 - z(X)) = 0
        //
        // Enable the permutation argument for only the rows involved.
        // degree (2 + input_degree + table_degree) or 4, whichever is larger:
        // (1 - (l_last(X) + l_blind(X))) * (
        //   z(\omega X) (a'(X) + \beta) (s'(X) + \gamma)
        //   - z(X) (\theta^{m-1} a_0(X) + ... + a_{m-1}(X) + \beta) (\theta^{m-1} s_0(X) + ... + s_{m-1}(X) + \gamma)
        // ) = 0
        //
        // The first two values of a' and s' should be the same.
        // degree 2:
        // l_0(X) * (a'(X) - s'(X)) = 0
        //
        // Either the two values are the same, or the previous
        // value of a' is the same as the current value.
        // degree 3:
        // (1 - (l_last(X) + l_blind(X))) * (a′(X) − s′(X))⋅(a′(X) − a′(\omega^{-1} X)) = 0
        let mut input_degree = 1;
        for expr in self.input_expressions.iter() {
            input_degree = std::cmp::max(input_degree, expr.degree());
        }
        let mut table_degree = 1;
        for expr in self.table_expressions.iter() {
            table_degree = std::cmp::max(table_degree, expr.degree());
        }

        // In practice because input_degree and table_degree are initialized to
        // one, the latter half of this max() invocation is at least 4 always,
        // rendering this call pointless except to be explicit in case we change
        // the initialization of input_degree/table_degree in the future.
        std::cmp::max(
            // (1 - (l_last + l_blind)) z(\omega X) (a'(X) + \beta) (s'(X) + \gamma)
            4,
            // (1 - (l_last + l_blind)) z(X) (\theta^{m-1} a_0(X) + ... + a_{m-1}(X) + \beta) (\theta^{m-1} s_0(X) + ... + s_{m-1}(X) + \gamma)
            2 + input_degree + table_degree,
        )
    }
}
