const codama = require("codama");
const anchorIdl = require("@codama/nodes-from-anchor");
const path = require("path");
const jsRenderers = require("@codama/renderers-js");
const rustRenderers = require("@codama/renderers-rust");

const projectRoot = path.join(__dirname, "..");
const idlDir = path.join(projectRoot, "idl");
const idl = require(path.join(idlDir, "svm_alm_controller.json"));
const rustClientsDir = path.join(__dirname, "..", "clients", "rust");
const tsClient = path.join(__dirname, "..", "clients", "ts");

const Codama = codama.createFromRoot(anchorIdl.rootNodeFromAnchor(idl));
// Rust Client
// Codama.accept(
//   rustRenderers.renderVisitor(path.join(rustClientsDir, "src", "generated"), {
//     formatCode: true,
//     crateFolder: rustClientsDir,
//     deleteFolderBeforeRendering: true,
//   })
// );
// TS Client
Codama.accept(
  jsRenderers.renderVisitor(path.join(tsClient, "src", "generated"), {
    nameTransformers: {},
  })
);
