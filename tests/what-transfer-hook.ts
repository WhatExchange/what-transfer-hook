import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { WhatTransferHook } from "../target/types/what_transfer_hook";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";

describe("what-transfer-hook", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.WhatTransferHook as Program<WhatTransferHook>;

  it("Is initialized!", async () => {
    // Add your test here.
    const privateKey = "fEwxtX3TZR4gwUB15bSqbZcWx5kJU6QsdoFR3RwVpdzRfcy8GDWAXVN8kJSbQUjfjGqJP19vsaJNmKwtH9zibth"
    const payer = Keypair.fromSecretKey(bs58.decode(privateKey));
    const mint = new PublicKey("HFhuKTC3snmUdyiav2vBU6gUemHXodJUdETkoVLoPU2U");

    const [whiteListPda, whiteListPdaBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from("white_list"),
      ],
      program.programId
    );

    const [extraAccountMetaListPda, _] = await PublicKey.findProgramAddress(
      [
        Buffer.from("extra-account-metas"),
        mint.toBuffer()
      ],
      program.programId
    );

    const tx = await program.methods.initializeExtraAccountMetaList(10).accounts({
      payer: payer.publicKey,
      extraAccountMetaList: extraAccountMetaListPda,
      mint,
      systemProgram: anchor.web3.SystemProgram.programId,
      whiteList: whiteListPda,
    }).signers([payer]).rpc();
    console.log("Your transaction signature", tx);
  });
});
