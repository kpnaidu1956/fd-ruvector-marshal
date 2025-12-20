import { useState } from 'react';
import { FileText, ChevronDown, ChevronUp, FileCode, FileSpreadsheet } from 'lucide-react';
import type { Citation } from '../api/types';

interface CitationCardProps {
  citation: Citation;
  index: number;
}

export function CitationCard({ citation, index }: CitationCardProps) {
  const [isExpanded, setIsExpanded] = useState(false);

  const getFileIcon = () => {
    const type = citation.file_type.toLowerCase();
    if (type === 'pdf' || type === 'docx') return <FileText className="w-4 h-4" />;
    if (type === 'csv' || type === 'xlsx') return <FileSpreadsheet className="w-4 h-4" />;
    if (type.startsWith('code') || type === 'txt' || type === 'markdown') return <FileCode className="w-4 h-4" />;
    return <FileText className="w-4 h-4" />;
  };

  const getLocationBadge = () => {
    if (citation.page_number) {
      return `Page ${citation.page_number}`;
    }
    if (citation.line_start && citation.line_end) {
      return `Lines ${citation.line_start}-${citation.line_end}`;
    }
    return null;
  };

  const formatSnippet = (text: string, highlighted: string) => {
    // If we have highlighted text, use it
    if (highlighted && highlighted !== text) {
      return <span dangerouslySetInnerHTML={{ __html: highlighted }} />;
    }
    return text;
  };

  const truncateSnippet = (text: string, maxLength: number = 200) => {
    if (text.length <= maxLength) return text;
    const truncated = text.slice(0, maxLength);
    const lastSpace = truncated.lastIndexOf(' ');
    return lastSpace > 0 ? truncated.slice(0, lastSpace) + '...' : truncated + '...';
  };

  return (
    <div className="bg-white border border-gray-200 rounded-lg overflow-hidden hover:border-gray-300 transition-colors">
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center gap-3 px-3 py-2 text-left hover:bg-gray-50 transition-colors"
      >
        <span className="flex-shrink-0 w-6 h-6 bg-primary-100 text-primary-700 rounded-full flex items-center justify-center text-xs font-medium">
          {index}
        </span>
        <span className="flex-shrink-0 text-gray-400">{getFileIcon()}</span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-gray-900 truncate">{citation.filename}</span>
            {getLocationBadge() && (
              <span className="flex-shrink-0 text-xs bg-gray-100 text-gray-600 px-2 py-0.5 rounded">
                {getLocationBadge()}
              </span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2 text-xs text-gray-500">
          <span
            className={`px-2 py-0.5 rounded ${
              citation.similarity_score >= 0.7
                ? 'bg-green-100 text-green-700'
                : citation.similarity_score >= 0.5
                ? 'bg-yellow-100 text-yellow-700'
                : 'bg-gray-100 text-gray-600'
            }`}
          >
            {Math.round(citation.similarity_score * 100)}% match
          </span>
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-gray-400" />
          ) : (
            <ChevronDown className="w-4 h-4 text-gray-400" />
          )}
        </div>
      </button>

      {/* Preview (always visible) */}
      {!isExpanded && (
        <div className="px-3 pb-2">
          <p className="text-sm text-gray-600 line-clamp-2">
            {truncateSnippet(citation.snippet)}
          </p>
        </div>
      )}

      {/* Expanded Content */}
      {isExpanded && (
        <div className="border-t border-gray-100">
          <div className="px-3 py-3">
            <p className="text-sm font-medium text-gray-700 mb-2">Source excerpt:</p>
            <div className="bg-gray-50 rounded-lg p-3 text-sm text-gray-700 leading-relaxed">
              {formatSnippet(citation.snippet, citation.snippet_highlighted)}
            </div>
          </div>

          {/* Metadata */}
          <div className="px-3 pb-3 flex flex-wrap gap-2 text-xs">
            {citation.section_title && (
              <span className="bg-blue-50 text-blue-700 px-2 py-1 rounded">
                Section: {citation.section_title}
              </span>
            )}
            <span className="bg-gray-100 text-gray-600 px-2 py-1 rounded">
              Type: {citation.file_type}
            </span>
            {citation.rerank_score && (
              <span className="bg-purple-50 text-purple-700 px-2 py-1 rounded">
                Rerank: {Math.round(citation.rerank_score * 100)}%
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
