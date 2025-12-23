import { useState, useEffect } from 'react';
import { FileUpload } from './components/FileUpload';
import { ChatInterface } from './components/ChatInterface';
import { DocumentList } from './components/DocumentList';
import { api } from './api/client';
import type { Document, QueryResponse } from './api/types';
import { FileText, MessageSquare, Upload, Database } from 'lucide-react';

type Tab = 'chat' | 'upload' | 'documents';

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('chat');
  const [documents, setDocuments] = useState<Document[]>([]);
  const [chatHistory, setChatHistory] = useState<Array<{
    type: 'user' | 'assistant';
    content: string;
    response?: QueryResponse;
  }>>([]);
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);

  // Check server health on load
  useEffect(() => {
    api.healthCheck().then(setIsHealthy);
    loadDocuments();
  }, []);

  const loadDocuments = async () => {
    try {
      const result = await api.listDocuments();
      setDocuments(result.documents);
    } catch (error) {
      console.error('Failed to load documents:', error);
    }
  };

  const handleUploadComplete = () => {
    loadDocuments();
  };

  const handleQuery = async (question: string) => {
    // Add user message
    setChatHistory(prev => [...prev, { type: 'user', content: question }]);

    try {
      const response = await api.query({
        question,
        top_k: 5,
        similarity_threshold: 0.3,
      });

      // Add assistant response
      setChatHistory(prev => [
        ...prev,
        { type: 'assistant', content: response.answer, response },
      ]);
    } catch (error) {
      setChatHistory(prev => [
        ...prev,
        {
          type: 'assistant',
          content: `Error: ${error instanceof Error ? error.message : 'Failed to get response'}`,
        },
      ]);
    }
  };

  const handleDeleteDocument = async (id: string) => {
    try {
      await api.deleteDocument(id);
      loadDocuments();
    } catch (error) {
      console.error('Failed to delete document:', error);
    }
  };

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white border-b border-gray-200">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-16">
            <div className="flex items-center gap-3">
              <Database className="w-8 h-8 text-primary-600" />
              <div>
                <h1 className="text-xl font-bold text-gray-900">RuVector RAG</h1>
                <p className="text-xs text-gray-500">Document Q&A with Citations</p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <span
                className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium ${
                  isHealthy === true
                    ? 'bg-green-100 text-green-700'
                    : isHealthy === false
                    ? 'bg-red-100 text-red-700'
                    : 'bg-gray-100 text-gray-700'
                }`}
              >
                {isHealthy === true ? 'Connected' : isHealthy === false ? 'Disconnected' : 'Checking...'}
              </span>
            </div>
          </div>
        </div>
      </header>

      {/* Navigation Tabs */}
      <div className="bg-white border-b border-gray-200">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <nav className="flex -mb-px">
            <button
              onClick={() => setActiveTab('chat')}
              className={`flex items-center gap-2 px-4 py-4 text-sm font-medium border-b-2 ${
                activeTab === 'chat'
                  ? 'border-primary-500 text-primary-600'
                  : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'
              }`}
            >
              <MessageSquare className="w-4 h-4" />
              Chat
            </button>
            <button
              onClick={() => setActiveTab('upload')}
              className={`flex items-center gap-2 px-4 py-4 text-sm font-medium border-b-2 ${
                activeTab === 'upload'
                  ? 'border-primary-500 text-primary-600'
                  : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'
              }`}
            >
              <Upload className="w-4 h-4" />
              Upload
            </button>
            <button
              onClick={() => setActiveTab('documents')}
              className={`flex items-center gap-2 px-4 py-4 text-sm font-medium border-b-2 ${
                activeTab === 'documents'
                  ? 'border-primary-500 text-primary-600'
                  : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'
              }`}
            >
              <FileText className="w-4 h-4" />
              Documents
              {documents.length > 0 && (
                <span className="ml-2 px-2 py-0.5 text-xs bg-gray-100 text-gray-600 rounded-full">
                  {documents.length}
                </span>
              )}
            </button>
          </nav>
        </div>
      </div>

      {/* Main Content */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {activeTab === 'chat' && (
          <ChatInterface
            chatHistory={chatHistory}
            onQuery={handleQuery}
            hasDocuments={documents.length > 0}
          />
        )}
        {activeTab === 'upload' && (
          <FileUpload onUploadComplete={handleUploadComplete} />
        )}
        {activeTab === 'documents' && (
          <DocumentList
            documents={documents}
            onDelete={handleDeleteDocument}
            onRefresh={loadDocuments}
          />
        )}
      </main>
    </div>
  );
}

export default App;
