import axios from "axios";
import { Octokit } from "@octokit/rest";
import * as dotenv from "dotenv";

dotenv.config();

// -------------------- Interfaces --------------------
interface GitCommitAuthor {
    name?: string | null;
    email?: string | null;
    date?: string | null;
}

interface GitCommitInfo {
    message: string;
    author?: GitCommitAuthor | null;
    committer?: GitCommitAuthor | null;
}

interface GitCommitFile {
    filename: string;
    patch?: string | null;
}

interface GitCommitResponse {
    commit: GitCommitInfo;
    files?: GitCommitFile[] | null;
}

interface EvaluateInput {
    githubHandle: string;
    repo: string;
    commits: string[];
    prompt: string;
}

interface EvaluationResult {
    summary: string;
    accuracyScore: number;
    reasoning: string;
}

// -------------------- GitHub Setup --------------------
const octokit = new Octokit({ auth: process.env.GITHUB_TOKEN });

// -------------------- Commit Fetch --------------------
async function getCommitDetails(owner: string, repo: string, commitSha: string): Promise<GitCommitResponse> {
    const { data } = await octokit.repos.getCommit({ owner, repo, ref: commitSha }) as { data: GitCommitResponse };

    console.log("\n🔹 Fetching commit:", commitSha);
    console.log("🧠 Message:", data.commit.message);
    console.log("👤 Author:", data.commit.author?.name ?? data.commit.committer?.name ?? "Unknown");
    console.log("📄 Files changed:");
    (data.files ?? []).forEach((file: GitCommitFile) => {
        console.log(`- ${file.filename}`);
        if (file.patch) console.log(file.patch.slice(0, 150));
    });

    return data;
}

// -------------------- Diff Aggregation --------------------
async function getDiffs(owner: string, repo: string, commits: string[]): Promise<string> {
    const diffs: string[] = [];

    for (const sha of commits) {
        const data = await getCommitDetails(owner, repo, sha);
        const diff = data.files?.map(f => f.patch).join("\n") || "";
        diffs.push(diff);
    }

    return diffs.join("\n");
}

// -------------------- Gemini Evaluation --------------------
async function evaluateTask(input: EvaluateInput): Promise<EvaluationResult> {
    const { githubHandle, repo, commits, prompt } = input;
    const diffs = await getDiffs(githubHandle, repo, commits);

    console.log("\n🤖 Sending diff summary request to Gemini...");

    const systemPrompt = `
You are an autonomous code-reviewing AI agent.

You will receive:
1. A natural-language task description that defines the intended work.
2. One or more Git commit diffs showing the actual implementation.

Your job:
- Summarize what was implemented in clear technical language.
- Compare the actual changes to the intended task.
- Return a JSON object with:
  {
    "summary": "what was done in code",
    "alignment": "describe how closely it matches the intended task",
    "accuracyScore": 0.0-1.0,
    "reasoning": "short explanation of score"
  }
Be concise and output valid JSON only.
`;

    const response = await axios.post(
        `${process.env.AI_MODEL}key=${process.env.AI_MODEL_API_KEY}`,
        {
            contents: [
                {
                    role: "user",
                    parts: [
                        { text: `${systemPrompt}\n\nTask description: ${prompt}\n\nHere are the diffs:\n${diffs}` }
                    ]
                }
            ]
        },
        {
            headers: {
                "Content-Type": "application/json",
            }
        }
    );

    const text: string =
        response.data?.candidates?.[0]?.content?.parts?.[0]?.text ?? "";

    try {
        const jsonStart = text.indexOf("{");
        const jsonEnd = text.lastIndexOf("}") + 1;
        const result: EvaluationResult = JSON.parse(text.slice(jsonStart, jsonEnd));
        return result;
    } catch (err) {
        console.error("⚠️ Could not parse Gemini output:", text);
        return {
            summary: "Parsing error",
            accuracyScore: 0,
            reasoning: "Model response invalid or malformed JSON.",
        };
    }
}

async function getRecentCommits(owner: string, repo: string, limit = 5): Promise<string[]> {
  const { data } = await octokit.repos.listCommits({
    owner,
    repo,
    per_page: limit,  // how many to fetch
  });

  const shas = data.map(commit => commit.sha);
  console.log(`📦 Found ${shas.length} recent commits:`, shas);
  return shas;
}

(async () => {
    const input: EvaluateInput = {
        githubHandle: "kumailnaqvi354",
        repo: "marinade-finance-integration-task",
        commits: await getRecentCommits("kumailnaqvi354", "marinade-finance-integration-task", 3),
        prompt: "Implement and test Marinade staking integration using Anchor and add test cases.",
    };

    const result = await evaluateTask(input);
    console.log("\n✅ Final Evaluation Result:");
    console.log(JSON.stringify(result, null, 2));
})();
