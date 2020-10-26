//! aggregated Schnorr {n,n}-Signatures
//!
//! See https://eprint.iacr.org/2018/068.pdf, https://eprint.iacr.org/2018/483.pdf subsection 5.1
use curv::{BigInt, FE, GE};

use curv::cryptographic_primitives::proofs::*;
use curv::elliptic::curves::traits::*;

use curv::cryptographic_primitives::hashing::hash_sha256::HSha256;
use curv::cryptographic_primitives::hashing::traits::*;

use curv::arithmetic::traits::Converter;
use curv::cryptographic_primitives::commitments::hash_commitment::HashCommitment;
use curv::cryptographic_primitives::commitments::traits::*;

const NUM_OF_SHARES: usize = 2;

#[derive(Debug)]
pub struct KeyPair {
    pub public_key: GE,
    private_key: FE,
}

impl KeyPair {
    pub fn create() -> KeyPair {
        let ec_point: GE = ECPoint::generator();
        let private_key: FE = ECScalar::new_random();
        let public_key = ec_point.scalar_mul(&private_key.get_element());
        KeyPair {
            public_key,
            private_key,
        }
    }

    pub fn create_from_private_key(private_key: &BigInt) -> KeyPair {
        let ec_point: GE = ECPoint::generator();
        let private_key: FE = ECScalar::from(private_key);
        let public_key = ec_point.scalar_mul(&private_key.get_element());
        KeyPair {
            public_key,
            private_key,
        }
    }
}

#[derive(Debug)]
pub struct KeyAgg {
    pub apk: GE,
    pub hash: BigInt,
}

impl KeyAgg {
    pub fn key_aggregation(my_pk: &GE, other_pk: &GE) -> KeyAgg {
        let hash = HSha256::create_hash(&[
            &BigInt::from(1),
            &my_pk.bytes_compressed_to_big_int(),
            &my_pk.bytes_compressed_to_big_int(),
            &other_pk.bytes_compressed_to_big_int(),
        ]);
        let hash_fe: FE = ECScalar::from(&hash);
        let a1 = my_pk.scalar_mul(&hash_fe.get_element());

        let hash2 = HSha256::create_hash(&[
            &BigInt::from(1),
            &other_pk.bytes_compressed_to_big_int(),
            &my_pk.bytes_compressed_to_big_int(),
            &other_pk.bytes_compressed_to_big_int(),
        ]);
        let hash2_fe: FE = ECScalar::from(&hash2);
        let a2 = other_pk.scalar_mul(&hash2_fe.get_element());
        let apk = a2.add_point(&(a1.get_element()));
        KeyAgg { apk, hash }
    }

    pub fn key_aggregation_n(pks: &[GE], party_index: usize) -> KeyAgg {
        let bn_1 = BigInt::from(1);
        let x_coor_vec: Vec<BigInt> = pks
            .iter()
            .map(|pk| pk.bytes_compressed_to_big_int())
            .collect();

        let hash_vec: Vec<BigInt> = x_coor_vec
            .iter()
            .map(|pk| {
                let mut vec = Vec::new();
                vec.push(&bn_1);
                vec.push(pk);
                for mpz in x_coor_vec.iter().take(pks.len()) {
                    vec.push(mpz);
                }
                HSha256::create_hash(&vec)
                //Doron: the "L" part of the hash
            })
            .collect();

        let mut apk_vec: Vec<GE> = pks
            .iter()
            .zip(&hash_vec)
            .map(|(pk, hash)| {
                let hash_t: FE = ECScalar::from(&hash);
                let pki: GE = pk.clone();
                pki.scalar_mul(&hash_t.get_element())
            })
            .collect();

        let pk1 = apk_vec.remove(0);
        let sum = apk_vec
            .iter()
            .fold(pk1, |acc, pk| acc.add_point(&pk.get_element()));

        KeyAgg {
            apk: sum,
            hash: hash_vec[party_index].clone(),
        }
    }
}

#[derive(Debug)]
pub struct EphemeralKey {
    pub keypair: KeyPair,
    pub commitment: BigInt,
    pub blind_factor: BigInt,
}

impl EphemeralKey {
    pub fn create_vec_from_private_key(x1: &KeyPair) -> Vec<EphemeralKey> {
        let mut EphermalKeys_vec: Vec<EphemeralKey> = vec![];
        for i in 0..NUM_OF_SHARES {
            let base_point: GE = ECPoint::generator();
            let hash_private_key_message =
                HSha256::create_hash(&[&x1.private_key.to_big_int(), &BigInt::from(i as i32)]);
            let ephemeral_private_key: FE = ECScalar::from(&hash_private_key_message);
            let ephemeral_public_key = base_point.scalar_mul(&ephemeral_private_key.get_element());
            let (commitment, blind_factor) = HashCommitment::create_commitment(
                &ephemeral_public_key.bytes_compressed_to_big_int(),
            );
            let eph_key = EphemeralKey {
                keypair: KeyPair {
                    public_key: ephemeral_public_key,
                    private_key: ephemeral_private_key,
                },
                commitment,
                blind_factor,
            };
            EphermalKeys_vec.push(eph_key);
        }
        EphermalKeys_vec
        //Doron: m,sk,g^sk ->  EphemeralKey{keypair{g^r,r}, H(g^r,nonce),nonce} where r= H(m,sk)
    }

    pub fn test_com(r_to_test: &GE, blind_factor: &BigInt, comm: &BigInt) -> bool {
        let computed_comm = &HashCommitment::create_commitment_with_user_defined_randomness(
            &r_to_test.bytes_compressed_to_big_int(),
            blind_factor,
        );
        computed_comm == comm
    }
}

pub struct Msg {
    first_msg: Vec<GE>,
    second_msg: Option<FE>,
}

pub struct State0 {
    pub keypair: KeyPair,
    pub ephk_vec: Vec<EphemeralKey>,
}
#[derive(Debug, Clone)]
pub struct State1 {
    pub R: GE,
    pub s_i: FE,
    pub c: BigInt,
    pub r_i: FE,
    pub b_coefficients: Vec<BigInt>,
}

impl State1 {
    fn get(&self) -> FE {
        self.s_i
    }
}

pub struct State {
    //    pub signature: FE,
    State0: State0,
    State1: Option<State1>,
    msg: Msg,
}

impl State {
    pub fn get_state_1(&self) -> &State1 {
        self.State1.as_ref().unwrap()
    }

    pub fn hash_0(r_hat: &GE, apk: &GE, message: &[u8], musig_bit: bool) -> BigInt {
        if musig_bit {
            HSha256::create_hash(&[
                &BigInt::from(0),
                &r_hat.x_coor().unwrap(),
                &apk.bytes_compressed_to_big_int(),
                &BigInt::from(message),
            ])
        } else {
            HSha256::create_hash(&[
                &r_hat.x_coor().unwrap(),
                &apk.bytes_compressed_to_big_int(),
                &BigInt::from(message),
            ])
        }
        //Doron: return H(0,R (=R1+R2),apk,m) or H(R,apk,m)
    }

    pub fn sign_0(
        &self,
        b_coefficients: &Vec<BigInt>,
        c: &BigInt,
        x: &KeyPair,
        a: &BigInt,
    ) -> (FE, FE) {
        let c_fe: FE = ECScalar::from(c);
        let a_fe: FE = ECScalar::from(a);
        let lin_comb_ephemeral_i: FE = self.State0.ephk_vec.
            iter().
            //  map(|emph| emph.keypair.private_key).
            // collect();
            //let lin_comb_ephemeral: FE = private_ephemeral_vec.
            //   iter().
            zip(b_coefficients).
            fold(ECScalar::zero(), |acc, (ephk,b)|
                acc + ephk.keypair.private_key * <FE as ECScalar<_>>::from(b));
        let s_fe = lin_comb_ephemeral_i.clone() + (c_fe * x.private_key.clone() * a_fe);
        (s_fe, lin_comb_ephemeral_i.clone())
        //Doron: sk,r,a,c->r+c*a*sk
    }

    pub fn add_ephemeral_keys(&mut self, msg_vec: &[Vec<GE>], party_index: usize) -> Vec<GE> {
        let mut R_j_vec: Vec<GE> = vec![];
        //println!("msg_vec {:?}",msg_vec);

        for j in 0..NUM_OF_SHARES {
            let pk_0j = self.State0.ephk_vec[j].keypair.public_key;
            //    println!("self_vec {:?}",pk_0j);

            let R_j: GE = msg_vec.
                iter().
                //  map(|emph| emph.first_msg.get(j)).
                fold(pk_0j, |acc, ephk| acc.add_point(&ephk.get(j).unwrap().get_element()));
            R_j_vec.push(R_j);
        }
        //  println!("R_j_vec {:?}",R_j_vec);
        R_j_vec
    }

    pub fn sign_1(x: KeyPair) -> State {
        let ephk_vec = EphemeralKey::create_vec_from_private_key(&x);
        let msg = ephk_vec
            .iter()
            .map(|eph_key| eph_key.keypair.public_key)
            .collect();
        State {
            State0: State0 {
                keypair: x,
                ephk_vec: ephk_vec,
            },
            State1: None,
            msg: Msg {
                first_msg: msg,
                second_msg: None,
            },
        }
    }

    pub fn get_pub_emph_keys(&self) -> Vec<BigInt> {
        self.State0
            .ephk_vec
            .iter()
            .map(|emph_key| emph_key.keypair.public_key.x_coor().unwrap())
            .collect()
    }

    pub fn get_msg_1(&self) -> &[GE] {
        &self.msg.first_msg
    }

    pub fn sign_2(
        &mut self,
        message: &[u8],
        pks: &Vec<GE>,
        msg_vec: Vec<Vec<GE>>,
        party_index: usize,
    ) -> (GE, GE) {
        let key_agg = KeyAgg::key_aggregation_n(&pks, party_index);
        let mut R_j_vec = self.add_ephemeral_keys(&msg_vec, party_index);
        println!("R_j_vec: {:?}", R_j_vec);
        let mut b_coefficients: Vec<BigInt> = Vec::new();
        b_coefficients.push(BigInt::from(1));

        for j in 1..NUM_OF_SHARES {
            let mut hnon_preimage: Vec<BigInt> = Vec::new();
            hnon_preimage.push(key_agg.apk.bytes_compressed_to_big_int());
            for i in 0..NUM_OF_SHARES {
                hnon_preimage.push(R_j_vec[i].bytes_compressed_to_big_int());
            }
            hnon_preimage.push(BigInt::from(message));
            hnon_preimage.push(BigInt::from(j as i32));
            //   let b_j = HSha256::create_hash(&hnon_preimage.iter().collect::<Vec<_>>());
            let b_j = HSha256::create_hash(&hnon_preimage.iter().collect::<Vec<_>>());
            b_coefficients.push(b_j);
            //            R = R.add_point(R_j_vec[j].scalar_mul(b_j));
        }
        let R_j0 = R_j_vec.remove(0);
        let mut b_coefficients_temp = b_coefficients.clone();
        let b_0 = b_coefficients_temp.remove(0);
        let R_0 = R_j0 * &<FE as ECScalar<_>>::from(&b_0);
        //   .scalar_mul(b_coefficients.remove(0));
        let R: GE = R_j_vec
            .iter()
            .zip(b_coefficients_temp.clone())
            .map(|(R_j, b_j)| R_j * &<FE as ECScalar<_>>::from(&b_j))
            .fold(R_0, |acc, R_j| acc.add_point(&R_j.get_element()));
        let c = State::hash_0(&R, &key_agg.apk, message, true);

        let (s_i, r_i) = self.sign_0(&b_coefficients, &c, &self.State0.keypair, &key_agg.hash);
        let base_point: GE = ECPoint::generator();
        let left_arg: GE = base_point * s_i;
        let pub_key = self.State0.keypair.public_key;
        let a_i: FE = ECScalar::from(&key_agg.hash);
        let c_fe: FE = ECScalar::from(&c);
        let right_arg: GE = pub_key * a_i * c_fe + base_point * r_i;
        // assert_eq!(left_arg,right_arg);
        println!("left_arg = {:?}", left_arg.get_element());
        println!("right_arg = {:?}", right_arg.get_element());
        self.State1 = Some(State1 {
            R,
            s_i,
            c,
            r_i,
            b_coefficients,
        });
        println!("state1 {:?}", self.State1);
        //    self
        (left_arg, right_arg)
    }

    pub fn sign_3(&self, msg_vec: &Vec<FE>) -> FE {
        let s_0 = self.State1.as_ref().unwrap().s_i;
        msg_vec.iter().fold(s_0, |acc, s_i| acc + s_i)
    }

    pub fn add_signature_parts(s1: BigInt, s2: &BigInt, r_tag: &GE) -> (BigInt, BigInt) {
        if *s2 == BigInt::from(0) {
            (r_tag.x_coor().unwrap(), s1)
        } else {
            let s1_fe: FE = ECScalar::from(&s1);
            let s2_fe: FE = ECScalar::from(&s2);
            let s1_plus_s2 = s1_fe.add(&s2_fe.get_element());
            (r_tag.x_coor().unwrap(), s1_plus_s2.to_big_int())
        }
    }
    //Doron s_1,s_2->s_1+s_2
}


pub fn verify_partial(
    signature: &FE,
    r_x: &BigInt,
    c: &FE,
    a: &FE,
    key_pub: &GE,
) -> Result<(), ProofError> {
    let g: GE = ECPoint::generator();
    let sG = g * signature;
    let cY = key_pub * a * c;
    let sG = sG.sub_point(&cY.get_element());
    if sG.x_coor().unwrap().to_hex() == *r_x.to_hex() {
        Ok(())
    } else {
        Err(ProofError)
    }
}


pub fn verify(
    signature: &FE,
    r_x: &BigInt,
    apk: &GE,
    c: &BigInt, //musig_bit: bool,
) -> Result<(), ProofError> {
    let base_point: GE = ECPoint::generator();
    //let signature_fe: FE =ECScalar::from(signature);
    let sG = base_point.scalar_mul(&signature.get_element());
    let c: FE = ECScalar::from(&c);
    let cY = apk.scalar_mul(&c.get_element());
    let sG = sG.sub_point(&cY.get_element());
    if sG.x_coor().unwrap().to_hex() == r_x.to_hex() {
        Ok(())
    } else {
        Err(ProofError)
    }
}


#[cfg(test)]
mod tests {
    use curv::{BigInt, FE, GE};
    use protocols::aggsig::musig_three_rounds::*;

    extern crate hex;

    use curv::elliptic::curves::traits::*;

    #[test]
    fn test_multiparty_signing_for_two_parties() {
        let is_musig = true;
        let message: [u8; 4] = [79, 77, 69, 82];

        // round 0: generate signing keys
        let party1_key = KeyPair::create();
        let party2_key = KeyPair::create();

        // round 1: send commitments to ephemeral public keys
        let party1_ephemeral_keys = EphemeralKey::create_vec_from_private_key(&party1_key);
        let party2_ephemeral_keys = EphemeralKey::create_vec_from_private_key(&party2_key);
        let mut vec_r_1 = vec![
            party1_ephemeral_keys[0].keypair.public_key,
            party1_ephemeral_keys[1].keypair.public_key,
        ];
        let mut vec_r_2 = vec![
            party2_ephemeral_keys[0].keypair.public_key,
            party2_ephemeral_keys[1].keypair.public_key,
        ];

        // compute apk:
        let mut pks: Vec<GE> = Vec::new();
        pks.push(party1_key.public_key.clone());
        pks.push(party2_key.public_key.clone());

        let party1_key_agg = KeyAgg::key_aggregation_n(&pks, 0);
        let party2_key_agg = KeyAgg::key_aggregation_n(&pks, 1);
        // compute R' = R1+R2:

        assert_eq!(party1_key_agg.apk, party2_key_agg.apk);
        let mut party_1 = State::sign_1(party1_key);
        let mut party_2 = State::sign_1(party2_key);

        let party1_first_msg = vec![Vec::from(party_1.get_msg_1())];
        let party2_first_msg = vec![Vec::from(party_2.get_msg_1())];

        let R1_vec: Vec<GE> = party_1.add_ephemeral_keys(&party2_first_msg, 0);
        let R2_vec: Vec<GE> = party_2.add_ephemeral_keys(&party1_first_msg, 1);

        assert_eq!(R1_vec, R2_vec);
        let (left_arg_partial, right_arg_partial) =
            party_1.sign_2(&message, &pks, party2_first_msg, 0);
        let (left_arg_partial, right_arg_partial) =
            party_2.sign_2(&message, &pks, party1_first_msg, 1);
        let base_point: GE = ECPoint::generator();

        let r_1: FE = party_1.get_state_1().r_i;
        let r_2: FE = party_2.get_state_1().r_i;

        let R_left = base_point * (r_1 + r_2);

        //  party_2.sign_2(&message, &pks, party1_first_msg, 1);
        let R1 = party_1.get_state_1().R;
        let R2 = party_2.get_state_1().R;
        assert_eq!(R1, R2);

        let s_1 = party_1.get_state_1().s_i;
        let s_2 = party_2.get_state_1().s_i;
        let c = party_1.get_state_1().c.clone();

//        assert!(verify_partial(&s_1,&R1.x_coor().unwrap(),c,))
        assert!(verify_partial(
            &ECScalar::from(&s_1.to_big_int()),
            &r_1.to_big_int(),
            &ECScalar::from(&c),
            &ECScalar::from(&party1_key_agg.hash),
            &party_1.State0.keypair.public_key
        )
            .is_ok());



        let s_total_1 = party_1.sign_3(&vec![s_2]);
        let s_total_2 = party_2.sign_3(&vec![s_1]);
        assert_eq!(s_total_1, s_total_2);

        assert!(verify(&s_total_1, &R1.x_coor().unwrap(), &party1_key_agg.apk, &c).is_ok());


    }
}
/*

assert_eq!(party1_r_tag, party2_r_tag);

// compute c = H0(Rtag || apk || message)
let party1_h_0 =
    EphemeralKey::hash_0(&party1_r_tag, &party1_key_agg.apk, &message, is_musig);
let party2_h_0 =
    EphemeralKey::hash_0(&party2_r_tag, &party2_key_agg.apk, &message, is_musig);
assert_eq!(party1_h_0, party2_h_0);

// compute partial signature s_i and send to the other party:
let s1 = EphemeralKey::sign(
    &party1_ephemeral_key,
    &party1_h_0,
    &party1_key,
    &party1_key_agg.hash,
);
let s2 = EphemeralKey::sign(
    &party2_ephemeral_key,
    &party2_h_0,
    &party2_key,
    &party2_key_agg.hash,
);

let r = party1_ephemeral_key.keypair.public_key.x_coor().unwrap();

assert!(verify_partial(
    &ECScalar::from(&s1),
    &r,
    &ECScalar::from(&party1_h_0),
    &ECScalar::from(&party1_key_agg.hash),
    &party1_key.public_key
)
    .is_ok());
// signature s:
let (r, s) = EphemeralKey::add_signature_parts(s1, &s2, &party1_r_tag);
// verify:
assert!(verify(&s, &r, &party1_key_agg.apk, &message, is_musig,).is_ok())
}

#[test]
fn test_schnorr_one_party() {
let is_musig = false;
let message: [u8; 4] = [79, 77, 69, 82];
let party1_key = KeyPair::create();
// let party1_key = KeyPair::create_from_private_key(&BigInt::from(259));
let party1_ephemeral_key = EphemeralKey::create_from_private_key(&party1_key, &message);

// compute c = H0(Rtag || apk || message)
let party1_h_0 = EphemeralKey::hash_0(
    &party1_ephemeral_key.keypair.public_key,
    &party1_key.public_key,
    &message,
    is_musig,
);

let s_tag = EphemeralKey::sign(
    &party1_ephemeral_key,
    &party1_h_0,
    &party1_key,
    &BigInt::from(1),
);

// signature s:
let (R, s) = EphemeralKey::add_signature_parts(
    s_tag,
    &BigInt::from(0),
    &party1_ephemeral_key.keypair.public_key,
);
// verify:
assert!(verify(&s, &R, &party1_key.public_key, &message, is_musig).is_ok());
}

//this test works only for curvesecp256k1
#[test]
fn test_schnorr_bip_test_vector_2() {
let private_key_raw = "B7E151628AED2A6ABF7158809CF4F3C762E7160F38B4DA56A784D9045190CFEF";
//let public_key_raw =  "03FAC2114C2FBB091527EB7C64ECB11F8021CB45E8E7809D3C0938E4B8C0E5F84B";
let message_raw = "243F6A8885A308D313198A2E03707344A4093822299F31D0082EFA98EC4E6C89";

let is_musig = false;
let message = hex::decode(message_raw).unwrap();
let party1_key = KeyPair::create_from_private_key(
    &BigInt::from_str_radix(&private_key_raw, 16).unwrap(),
);
let party1_ephemeral_key = EphemeralKey::create_from_private_key(&party1_key, &message);

// compute c = H0(Rtag || apk || message)
let party1_h_0 = EphemeralKey::hash_0(
    &party1_ephemeral_key.keypair.public_key,
    &party1_key.public_key,
    &message,
    is_musig,
);

// compute partial signature s_i and send to the other party:
let s_tag = EphemeralKey::sign(
    &party1_ephemeral_key,
    &party1_h_0,
    &party1_key,
    &BigInt::from(1),
);

// signature s:
let (R, s) = EphemeralKey::add_signature_parts(
    s_tag,
    &BigInt::from(0),
    &party1_ephemeral_key.keypair.public_key,
);

let test_vector_R =
    "2a298dacae57395a15d0795ddbfd1dcb564da82b0f269bc70a74f8220429ba1d".to_string();
let test_vector_s =
    "1e51a22ccec35599b8f266912281f8365ffc2d035a230434a1a64dc59f7013fd".to_string();
let sig_R = R.to_str_radix(16);
let sig_s = s.to_str_radix(16);
assert_eq!(test_vector_R, sig_R);
assert_eq!(test_vector_s, sig_s);
// verify:
assert!(verify(&s, &R, &party1_key.public_key, &message, is_musig).is_ok())
}
}
*/
