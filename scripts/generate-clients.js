const codama = require("codama");
const anchorIdl = require("@codama/nodes-from-anchor");
const path = require("path");
const renderers = require("@codama/renderers");

const projectRoot = path.join(__dirname, "..");
const idlDir = path.join(projectRoot, "idl");
const idl = require(path.join(idlDir, "svm_alm_controller.json"));
const rustClientsDir = path.join(__dirname, "..", "clients", "rust");

const Codama = codama.createFromRoot(anchorIdl.rootNodeFromAnchor(idl));
Codama.accept(
  renderers.renderRustVisitor(path.join(rustClientsDir, "src", "generated"), {
    formatCode: true,
    crateFolder: rustClientsDir,
    deleteFolderBeforeRendering: true,
  })
);
