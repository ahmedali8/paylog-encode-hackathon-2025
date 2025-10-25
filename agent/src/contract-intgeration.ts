//call agent function to get the result
// create a hash of the result and the diffs
// call the command to sh and the hash init

import { evaluateTask, getRecentCommits } from "./index.ts";
import crypto from "crypto";
import fs from "fs";

interface EvaluateInput {
  githubHandle: string;
  repo: string;
  commits: string[];
  prompt: string;
}

async function evaluateTaskAndCreateHash(): Promise<void> {
  try {
    console.log("\n🔹 Evaluating task and creating hash...");

    // Check for required environment variables
    if (!process.env.GITHUB_TOKEN) {
      console.error("❌ Error: GITHUB_TOKEN environment variable is not set");
      return;
    }

    if (!process.env.AI_MODEL || !process.env.AI_MODEL_API_KEY) {
      console.error("❌ Error: AI_MODEL or AI_MODEL_API_KEY environment variables are not set");
      return;
    }

    console.log("✅ Environment variables are set");

    const input: EvaluateInput = {
      githubHandle: "kumailnaqvi354",
      repo: "marinade-finance-integration-task",
      commits: await getRecentCommits("kumailnaqvi354", "marinade-finance-integration-task", 3),
      prompt: "Implement and test Marinade staking integration using Anchor and add test cases.",
    };

    const result = await evaluateTask(input);
    console.log("\n✅ Final Evaluation Result:");
    console.log(JSON.stringify(result, null, 2));

    // Create hash and convert to U8 array
    const hash = crypto.createHash("sha256").update(JSON.stringify(result)).digest("hex");
    console.log("\n✅ Hash (hex):", hash);

    // Convert hex string to U8 array
    const hashU8Array = new Uint8Array(hash.match(/.{1,2}/g)!.map((byte) => parseInt(byte, 16)));
    console.log("\n✅ Hash (U8 array):", Array.from(hashU8Array));

    // Read existing JSON file
    const outputPath = "../smart-contract/schema/request.json";
    let existingData = {};

    // Ensure directory exists
    const dirPath = "../smart-contract/schema";
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
      console.log("\n✅ Created directory structure");
    }

    try {
      const fileContent = fs.readFileSync(outputPath, "utf8");
      existingData = JSON.parse(fileContent);
      console.log("\n✅ Read existing JSON file");
    } catch (error) {
      console.log("\n⚠️ File doesn't exist or is empty, creating new structure");
    }

    // Update only the work_hash field
    const updatedData = {
      ...existingData,
      work_hash: Array.from(hashU8Array),
    };

    // Save updated JSON file
    fs.writeFileSync(outputPath, JSON.stringify(updatedData, null, 2));
    console.log(`\n✅ Updated work_hash in: ${outputPath}`);
  } catch (error) {
    console.error("❌ Error during evaluation:", error);
  }
}

(async () => {
  await evaluateTaskAndCreateHash();
})();
