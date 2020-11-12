(ns vogelsang.db
  (:require
   [clojure.java.io :as io]
   [datalevin.core :as d]
   [java-time :as jt]
   [taoensso.nippy :as nippy])) 

(nippy/extend-freeze
  java.time.LocalDate :java.time/LocalDate
  [x data-output]
  (.writeUTF data-output (jt/format x)))

(nippy/extend-thaw
  :java.time/LocalDate
  [data-input]
  (jt/local-date (.readUTF data-input)))

(def schema
  {:vogelsang.data/symbols {:db/cardinality :db.cardinality/many}
   :vogelsang/id           {:db/unique :db.unique/identity}
   :yf/symbol              {:db/valueType :db.type/string}
   :yf.quotes/symbol       {:db/valueType :db.type/string}
   :yf.quotes/date         {:db/valueType :db.type/string}
   :yf.quotes/open         {:db/valueType :db.type/double}
   :yf.quotes/high         {:db/valueType :db.type/double}
   :yf.quotes/low          {:db/valueType :db.type/double}
   :yf.quotes/close        {:db/valueType :db.type/double}
   :yf.quotes/volume       {:db/valueType :db.type/double}
   })

(def conn (d/create-conn "./db" schema))

(defn delete-db-files! []
  (doseq [f (file-seq (io/as-file "./db"))]
    (when (not (.isDirectory f)) (io/delete-file f))))

(defn reset-db! []
  (delete-db-files!)
  (reset! conn @(d/create-conn "./db" schema)))
