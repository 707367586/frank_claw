import KnowledgeSourceList from "../components/KnowledgeSourceList";
import KnowledgeSearchPanel from "../components/KnowledgeSearchPanel";

export default function KnowledgePage() {
  return (
    <div className="knowledge-page">
      <aside className="knowledge-page__left"><KnowledgeSourceList /></aside>
      <section className="knowledge-page__right"><KnowledgeSearchPanel /></section>
    </div>
  );
}
