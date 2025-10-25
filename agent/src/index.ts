import express from "express";
import { Webhooks, createNodeMiddleware } from "@octokit/webhooks";
import { Octokit } from "@octokit/rest";
import OpenAI from "openai";
import dotenv from "dotenv";
import crypto from "crypto";

dotenv.config();

// Provide a simple declaration so TypeScript recognizes the Node global `process`
// without requiring project-wide type installation.
declare const process: any;

const app = express();
const port = 3000;

const webhooks = new Webhooks({
  secret: process.env.GITHUB_WEBHOOK_SECRET!,
});

const openai = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
const octokit = new Octokit();

webhooks.on("push", async ({ payload }) => {
  const repo = payload.repository.full_name;
  const base = payload.before;
  const head = payload.after;

  console.log(`🔔 Push event received for ${repo}`);

  if (!payload.repository.owner) {
    console.error("Missing repository owner in webhook payload");
    return;
  }

  const { data } = await octokit.repos.compareCommitsWithBasehead({
    owner: payload.repository.owner.login ?? payload.repository.owner.name ?? "",
    repo: payload.repository.name,
    basehead: `${base}...${head}`,
  });

  const diff = data.files?.map(f => f.patch).join("\n") || "";

  const summary = await openai.chat.completions.create({
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: `Summarize this diff:\n${diff}` }],
  });

  const work_hash = crypto.createHash("sha256")
    .update(diff)
    .digest("hex");

  console.log("🧠 Summary:", summary.choices[0].message.content);
  console.log("🔐 work_hash:", work_hash);
});

app.use("/webhook", createNodeMiddleware(webhooks));
app.listen(port, () => console.log(`🚀 Agent running at http://localhost:${port}`));
