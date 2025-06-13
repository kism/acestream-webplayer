function getStream(streamId) {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 1000); // 1 second timeout:

  // GET the hello endpoint that the flask app has
  return fetch(`/api/stream/${streamId}`, {
    method: "GET",
    signal: controller.signal,
  })
    .then((response) => {
      // Check if the request was successful (status code 200)
      if (response.ok) {
        return response.json(); // We interoperate the response as json and pass it along...
      }
    })
    .then((data) => {
      // console.log(`Stream data: ${JSON.stringify(data)}`);
      return data; // Return the data to the caller
    })
    .catch((error) => {
      clearTimeout(timeoutId);
      if (error.name === "AbortError") {
        console.error("Fetch request timed out");
      } else {
        console.error(`Error: ${error.message}`);
      }
      throw error; // Re-throw to allow caller to handle
    });
}

function flashBackgroundColor(element, state, duration = 200) {
  let backgroundClass = 'status-neutral-background';
  if (state === "good") {
    backgroundClass = 'status-good-background';
  } else if (state === "bad") {
    backgroundClass = 'status-bad-background';
  }

  element.classList.add(backgroundClass);
  setTimeout(() => {
    element.classList.remove(backgroundClass);
  }, duration);
}

function getStreams() {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 1000); // 1 second timeout:

  fetch("/api/streams/flat", {
    method: "GET",
    signal: controller.signal,
  })
    .then((response) => {
      if (response.ok) {
        return response.json();
      } else {
        const streamStatus = document.getElementById("stream-status");
        setStatusClass(streamStatus, "bad");
        streamStatus.innerHTML = `API FAILURE ${response.status}`;

        throw new Error(`Error fetching data. Status code: ${response.status}`);
      }
    })
    .then((data) => {
      const table = document.getElementById("stream-table");
      table.innerHTML = ""; // Clear the table before adding new data
      const tr_heading = document.createElement("tr");

      const th_quality = document.createElement("th");
      th_quality.textContent = "Quality";
      tr_heading.appendChild(th_quality);
      flashBackgroundColor(th_quality);

      const th_link = document.createElement("th");
      th_link.textContent = "Stream";
      tr_heading.appendChild(th_link);
      flashBackgroundColor(th_link);

      const th_source = document.createElement("th");
      th_source.textContent = "Source";
      flashBackgroundColor(th_source);

      tr_heading.appendChild(th_source);
      table.appendChild(tr_heading);

      // Sort the data by quality in descending order
      data.sort((a, b) => b.quality - a.quality);

      for (const stream of data) {
        const tr = document.createElement("tr");

        // Quality cell
        const td_quality = document.createElement("td");
        td_quality.textContent = `${stream.quality}`;
        if (stream.quality === -1) {
          setStatusClass(td_quality, "neutral");
          td_quality.textContent = "?";
        } else if (stream.quality < 20) {
          setStatusClass(td_quality, "bad");
        } else if (stream.quality >= 20 && stream.quality <= 80) {
          setStatusClass(td_quality, "neutral");
        } else if (stream.quality >= 80) {
          setStatusClass(td_quality, "good");
        }
        td_quality.classList.add("quality-code");

        // Link cell
        td_link = document.createElement("td");
        const a = document.createElement("a"); // Create a new anchor element
        a.textContent = `${stream.title}`;
        a.onclick = () => {
          loadStreamUrl(stream.ace_id, stream.title); // Set the onclick event to load the stream URL
          const video = document.getElementById("video");
          video.play().catch((error) => {
            console.error("Error playing video:", error);
            setOnPageErrorMessage("Error playing video");
          });
        };
        td_link.appendChild(a); // Append the anchor element to the table data cell

        // Source Cell
        td_source = document.createElement("td");
        td_source.textContent = `${stream.site_name}`;

        // Append to the row
        tr.appendChild(td_quality);
        tr.appendChild(td_link);
        tr.appendChild(td_source);
        table.appendChild(tr);
      }
    })
    .catch((error) => {
      clearTimeout(timeoutId); //Stop the timeout since we only care about the GET timing out
      if (error.name === "AbortError") {
        console.error("Fetch request timed out");
        const streamStatus = document.getElementById("stream-status");
        setStatusClass(streamStatus, "bad");
        streamStatus.innerHTML = `API FAILURE: Fetch Timeout`;
      } else {
        console.error(`Error: ${error.message}`);
      }
    });
}

function setStatusClass(element, status) {
  // Remove all status classes from the element
  const statusClasses = ["status-good", "status-neutral", "status-bad"];
  statusClasses.forEach((cls) => {
    element.classList.remove(cls);
  });
  // Add the appropriate status class based on the status parameter
  if (status === "good") {
    element.classList.add("status-good");
  } else if (status === "neutral") {
    element.classList.add("status-neutral");
  } else if (status === "bad") {
    element.classList.add("status-bad");
  }
}

function setOnPageErrorMessage(message) {
  const streamStatus = document.getElementById("stream-status");
  streamStatus.innerHTML = message;
  setStatusClass(streamStatus, "bad");

  flashBackgroundColor(streamStatus, "bad", 500); // Flash the background color to indicate an error
}

function loadStream() {
  const video = document.getElementById("video");
  const videoSrc = `/hls/${window.location.hash.substring(1)}`;
  console.log(`Loading stream: ${videoSrc}`);

  const streamDirectUrl = document.getElementById("stream-url");
  streamDirectUrl.innerHTML = `${window.location.origin}${videoSrc}`;

  const streamStatus = document.getElementById("stream-status");
  setStatusClass(streamStatus, "neutral");
  streamStatus.innerHTML = `Stream loaded.`;

  if (Hls.isSupported()) {
    var hls = new Hls();

    // Add error event listener
    hls.on(Hls.Events.ERROR, function (event, data) {
      console.error("HLS error:", data);
      let errorMessage = "Stream loading failed";

      if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
        errorMessage = "Network error: Ace doen't have the stream segment";
      } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
        errorMessage = "Media error: Stream not ready";
      } else if (data.type === Hls.ErrorTypes.MUX_ERROR) {
        errorMessage = "Stream parsing error";
      }
      setOnPageErrorMessage(errorMessage);
    });

    // Wait for HLS to be ready before allowing play
    hls.on(Hls.Events.MANIFEST_PARSED, function () {
      console.log("HLS manifest parsed, stream ready to play");
    });

    hls.loadSource(videoSrc);
    hls.attachMedia(video);
  } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
    video.src = videoSrc; // For Safari

    // Add error event listener for Safari
    video.addEventListener("error", function (e) {
      console.error("Video error:", e);
      setOnPageErrorMessage("Stream loading failed");
    });
  } else {
    console.error("This browser does not support HLS playbook.");
    setOnPageErrorMessage("This browser does not support HLS playbook.");
    return;
  }
}

function loadStreamUrl(streamId, streamName) {
  window.location.hash = streamId; // Set the URL in the hash
  document.getElementById("stream-name").innerText = streamName;
  loadStream();
}

function checkIfPlaying() {
  const video = document.getElementById("video");
  if (!video.paused && !video.ended && video.currentTime > 0 && video.readyState > 2) {
    const streamStatus = document.getElementById("stream-status");
    streamStatus.innerHTML = "Playing"; // Hide the status element if the video is playing
    setStatusClass(streamStatus, "good");
  }
}

function togglePlayerSize() {
  const video = document.getElementById("video");
  const playerContainer = document.getElementById("player-container");

  if (video.style.width === "100%") {
    video.style.width = "640px"; // Set to original width
    video.style.height = "360px"; // Set to original height
    playerContainer.style.width = "640px"; // Set container width
    playerContainer.style.height = "360px"; // Set container height
  } else {
    video.style.width = "100%"; // Set to full width
    video.style.height = "100%"; // Set to full height
    playerContainer.style.width = "100%"; // Set container width
    playerContainer.style.height = "100%"; // Set container height
  }
}

function loadPlayStream(streamID) {
  getStream(streamID)
    .then((streamInfo) => {
      loadStreamUrl(streamID, streamInfo.title);

      // Wait a bit for the stream to load, then play
      setTimeout(() => {
        const video = document.getElementById("video");
        video.play().catch((error) => {
          console.error("Error playing video:", error);
          setOnPageErrorMessage("Error playing video");
        });
      }, 1000);
    })
    .catch((error) => {
      console.error("Failed to get stream info:", error);
      loadStreamUrl(streamID, streamID);
    });
}

function loadPlayStreamFromHash() {
  const streamId = window.location.hash.substring(1);
  if (streamId) {
    console.log(`Loading stream from hash: ${streamId}`);
    loadPlayStream(streamId);
  } else {
    console.error("No stream loaded.");
    setOnPageErrorMessage("No stream loaded.");
  }
}

// Wrap DOM-dependent code in DOMContentLoaded event
document.addEventListener("DOMContentLoaded", function () {
  const streamStatus = document.getElementById("stream-status");
  setStatusClass(streamStatus, "neutral");
  streamStatus.innerHTML = "Ready to load a stream";

  getStreams();

  window.addEventListener("loadStream", loadStream);

  let streamId = window.location.hash.substring(1);

  if (streamId) {
    console.log(`Loading stream on page load: ${streamId}`);
    getStream(streamId)
      .then((streamInfo) => {
        console.log(`Stream info: ${JSON.stringify(streamInfo)}`);
        loadStreamUrl(streamId, streamInfo.title);
      })
      .catch((error) => {
        console.error("Failed to get stream info:", error);
        loadStreamUrl(streamId, streamId); // Fallback to streamId as title
      });
  }

  setInterval(checkIfPlaying, 1000); // Check every second if the video is playing

  loadStreamButton = document.getElementById("load-stream-button");
  loadStreamButton.onclick = () => {
    const streamId = document.getElementById("stream-id-input").value; // Get the value from the input field
    if (streamId) {
      loadPlayStream(streamId);
    }
  };
});
