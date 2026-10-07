/**
 * Writes `<app>/inputs.lock` for every release-please package: hashes of the app's build inputs
 * that live outside its directory.
 *
 * release-please attributes commits to apps by path only. A change to a shared input (libs/runtime,
 * a lockfile bump, a workspace manifest) must therefore touch each affected app's directory, or the
 * app never releases it. Regenerating rewrites exactly the affected apps' files, and the commit then
 * counts towards their next release.
 *
 *   bun run inputs          # rewrite stale files
 *   bun run inputs --check  # CI: exit 1 if any file is stale
 *
 * Per app:
 * - cargo: crates reachable from the app's crates over normal and build edges (`cargo metadata`), with
 *   their resolved features. Apps with a Dockerfile resolve for linux only. Features are unified across
 *   the whole Cargo workspace, so a feature change in one app can mark its workspace siblings too.
 * - bun: packages reachable from the app's Bun workspaces in bun.lock, dev dependencies included.
 * - path: sources of each local crate or workspace outside the app that either closure reaches.
 * - file: .dockerignore for apps with a Dockerfile; rust-toolchain.toml for Rust apps built on the host.
 *
 * The app's own crates and workspaces contribute neither version nor sources: both live inside the
 * app, and release-please bumps the versions in every release.
 */
import { $ } from "bun";
import { basename, dirname, join, relative, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const LINUX = "x86_64-unknown-linux-gnu";
const FILE = "inputs.lock";
const check = process.argv.includes("--check");

type CargoMetadata = {
  packages: {
    id: string;
    name: string;
    version: string;
    source: string | null;
    edition: string;
    manifest_path: string;
  }[];
  workspace_members: string[];
  resolve: {
    nodes: {
      id: string;
      features: string[];
      deps: { pkg: string; dep_kinds: { kind: string | null }[] }[];
    }[];
  };
};
type Deps = Record<string, string>;
type BunWorkspace = {
  name: string;
  dependencies?: Deps;
  devDependencies?: Deps;
  optionalDependencies?: Deps;
  peerDependencies?: Deps;
};
type BunLock = {
  workspaces: Record<string, BunWorkspace>;
  packages: Record<string, unknown[]>;
};

const inside = (path: string, dir: string) =>
  path === dir || path.startsWith(`${dir}/`);
const fromRoot = (path: string) => relative(ROOT, path);
const digest = (text: string) =>
  new Bun.CryptoHasher("sha256").update(text).digest("hex").slice(0, 16);
const exists = (path: string) => Bun.file(join(ROOT, path)).exists();

async function run(command: string[]): Promise<string> {
  const result = await $`${command}`.cwd(ROOT).quiet().nothrow();
  if (result.exitCode !== 0) {
    throw new Error(
      `${command.join(" ")} failed:\n${result.stderr.toString()}`,
    );
  }
  return result.stdout.toString();
}

/** Git blob ids, so line-ending conversion on checkout never changes a hash. */
async function hashFiles(paths: string[]): Promise<string> {
  const listed = (
    await run([
      "git",
      "ls-files",
      "-z",
      "-co",
      "--exclude-standard",
      "--",
      ...paths,
    ])
  )
    .split("\0")
    .filter(Boolean)
    .sort();
  const files: string[] = [];
  for (const file of listed) {
    // Listed but deleted in the working tree.
    if (await exists(file)) files.push(file);
  }
  if (files.length === 0)
    throw new Error(`no files found in ${paths.join(", ")}`);
  const result =
    await $`git hash-object --stdin-paths < ${new Response(files.join("\n"))}`
      .cwd(ROOT)
      .quiet();
  const blobs = result.stdout.toString().trim().split("\n");
  return digest(files.map((file, i) => `${file} ${blobs[i]}`).join("\n"));
}

const metadataCache = new Map<string, Promise<CargoMetadata>>();
function cargoMetadata(
  manifest: string,
  platform?: string,
): Promise<CargoMetadata> {
  const key = `${manifest} ${platform}`;
  let metadata = metadataCache.get(key);
  if (!metadata) {
    const args = [
      "cargo",
      "metadata",
      "--format-version",
      "1",
      "--locked",
      "--manifest-path",
      manifest,
    ];
    if (platform) args.push("--filter-platform", platform);
    metadata = run(args).then((out) => JSON.parse(out));
    metadataCache.set(key, metadata);
  }
  return metadata;
}

/** The app's own Cargo workspace, else the root workspace members inside the app. */
async function cargoInputs(app: string, platform: string | undefined) {
  const own = `${app}/Cargo.toml`;
  const ownWorkspace =
    (await exists(own)) &&
    "workspace" in Bun.TOML.parse(await Bun.file(join(ROOT, own)).text());
  const manifest = ownWorkspace ? own : "Cargo.toml";
  const metadata = await cargoMetadata(manifest, platform);

  const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const dirOf = (id: string) =>
    fromRoot(dirname(packages.get(id)!.manifest_path));

  const roots = metadata.workspace_members.filter((id) =>
    inside(dirOf(id), app),
  );
  if (roots.length === 0) return undefined;

  const reached = new Set(roots);
  for (const id of reached) {
    for (const dep of nodes.get(id)!.deps) {
      if (dep.dep_kinds.some(({ kind }) => kind !== "dev"))
        reached.add(dep.pkg);
    }
  }

  const entries: string[] = [];
  const paths: string[] = [];
  for (const id of reached) {
    const pkg = packages.get(id)!;
    const features = nodes.get(id)!.features.toSorted().join(",");
    if (pkg.source !== null) {
      entries.push(`${pkg.name} ${pkg.version} ${pkg.source} ${features}`);
    } else if (inside(dirOf(id), app)) {
      entries.push(`${pkg.name} ${pkg.edition} ${features}`);
    } else {
      entries.push(`${pkg.name} ${pkg.version} ${pkg.edition} ${features}`);
      paths.push(dirOf(id));
    }
  }
  // Profiles apply from the workspace root only: the app's own, or the shared root manifest.
  if (!ownWorkspace) {
    const { profile } = Bun.TOML.parse(
      await Bun.file(join(ROOT, manifest)).text(),
    ) as { profile?: unknown };
    entries.push(`profile ${JSON.stringify(profile ?? {})}`);
  }
  return {
    hash: digest(entries.toSorted().join("\n")),
    count: reached.size,
    paths,
  };
}

const lock: BunLock = Bun.JSONC.parse(
  await Bun.file(join(ROOT, "bun.lock")).text(),
);

/** `@scope/a/b/@scope/c` → `["@scope/a", "b", "@scope/c"]`: a bun.lock key's nesting chain. */
function chain(key: string): string[] {
  const parts = key.split("/");
  const names: string[] = [];
  for (let i = 0; i < parts.length; i++) {
    names.push(
      parts[i]!.startsWith("@") ? `${parts[i]}/${parts[++i]}` : parts[i]!,
    );
  }
  return names;
}

/** Node resolution over bun.lock keys: nearest nested copy first, hoisted copy last. */
function resolveKey(from: string[], name: string): string | undefined {
  for (let depth = from.length; depth >= 0; depth--) {
    const key = [...from.slice(0, depth), name].join("/");
    if (key in lock.packages) return key;
  }
}

function bunInputs(app: string) {
  const entries = new Set<string>();
  const paths = new Set<string>();
  const seen = new Set<string>();
  const queue: { from: string[]; required: Deps; optional: Deps }[] = [];

  const visitWorkspace = (from: string[], workspace: BunWorkspace) =>
    queue.push({
      from,
      required: { ...workspace.dependencies, ...workspace.devDependencies },
      optional: {
        ...workspace.optionalDependencies,
        ...workspace.peerDependencies,
      },
    });

  for (const [path, workspace] of Object.entries(lock.workspaces)) {
    if (path && inside(path, app)) visitWorkspace([workspace.name], workspace);
  }
  if (queue.length === 0) return undefined;

  for (const { from, required, optional } of queue) {
    for (const [name, optionalDep] of [
      ...Object.keys(required).map((name) => [name, false] as const),
      ...Object.keys(optional).map((name) => [name, true] as const),
    ]) {
      const key = resolveKey(from, name);
      if (!key) {
        if (optionalDep) continue;
        throw new Error(
          `bun.lock: cannot resolve ${name} from ${from.join("/") || "root"}`,
        );
      }
      if (seen.has(key)) continue;
      seen.add(key);

      const entry = lock.packages[key]!;
      const ident = entry[0] as string;
      const workspacePath = ident.split("@workspace:")[1];
      if (workspacePath !== undefined) {
        if (!inside(workspacePath, app)) paths.add(workspacePath);
        visitWorkspace(chain(key), lock.workspaces[workspacePath]!);
        continue;
      }
      const meta = (entry.find(
        (part) => typeof part === "object" && part !== null,
      ) ?? {}) as BunWorkspace;
      const integrity =
        entry.length > 1 && typeof entry.at(-1) === "string"
          ? entry.at(-1)
          : "";
      entries.add(`${ident} ${integrity}`);
      queue.push({
        from: chain(key),
        required: { ...meta.dependencies },
        optional: { ...meta.optionalDependencies, ...meta.peerDependencies },
      });
    }
  }
  return {
    hash: digest([...entries].sort().join("\n")),
    count: entries.size,
    paths: [...paths],
  };
}

async function render(app: string, component: string): Promise<string> {
  const docker = await exists(`${app}/Dockerfile`);
  const cargo = await cargoInputs(app, docker ? LINUX : undefined);
  const bun = bunInputs(app);

  const lines = [
    "# Generated by `bun run inputs`; do not edit. CI fails when stale.",
    `# Build inputs of ${component} outside ${app}/. A change here releases ${component}.`,
  ];
  if (cargo) lines.push(`cargo ${cargo.hash} ${cargo.count}`);
  if (bun) lines.push(`bun ${bun.hash} ${bun.count}`);
  for (const path of [
    ...new Set([...(cargo?.paths ?? []), ...(bun?.paths ?? [])]),
  ].sort()) {
    lines.push(`path ${path} ${await hashFiles([path])}`);
  }
  const files = docker
    ? [".dockerignore"]
    : cargo
      ? ["rust-toolchain.toml"]
      : [];
  for (const file of files)
    lines.push(`file ${file} ${await hashFiles([file])}`);
  return `${lines.join("\n")}\n`;
}

const config: { packages: Record<string, { component?: string }> } =
  await Bun.file(join(ROOT, "release-please-config.json")).json();

const stale: string[] = [];
for (const [app, { component = basename(app) }] of Object.entries(
  config.packages,
)) {
  const path = `${app}/${FILE}`;
  const expected = await render(app, component);
  const file = Bun.file(join(ROOT, path));
  const actual = (await file.exists()) ? await file.text() : "";
  if (actual === expected) continue;
  stale.push(path);
  if (check) {
    console.error(`${path} is stale. Expected:\n${expected}`);
  } else {
    await Bun.write(file, expected);
    console.log(`updated ${path}`);
  }
}

if (check && stale.length > 0) {
  console.error(
    "Run `bun run inputs` and commit the result: see docs/releases.md.",
  );
  process.exit(1);
}
